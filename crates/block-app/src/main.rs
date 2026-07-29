mod editor;
mod index;

use std::{
    collections::HashMap, error::Error, io, net::TcpListener as StdTcpListener, path::PathBuf,
    thread, time::Duration,
};

use block::Block;
use block_client::{text::TextDocument, BlockClient, BlockHandle};
use editor::{BlockEditor, EditorRegistry};
use eframe::egui;
use index::{BlockEntry, WorkspaceIndex, WorkspaceIndexOperation, WORKSPACE_INDEX_ID};
use tokio::net::TcpListener;
use uuid::Uuid;

const APP_ID: &str = "Block";
const DEFAULT_TITLE: &str = "Untitled";

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID)
            .with_inner_size([1100.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        APP_ID,
        options,
        Box::new(|_| {
            BlockApp::new()
                .map(|app| Box::new(app) as Box<dyn eframe::App>)
                .map_err(Into::into)
        }),
    )
}

struct BlockApp {
    client: BlockClient,
    workspace: BlockHandle<WorkspaceIndex>,
    registry: EditorRegistry,
    editors: HashMap<Uuid, Box<dyn BlockEditor>>,
    tabs: Vec<Uuid>,
    active_tab: Option<Uuid>,
    rename: Option<RenameState>,
}

struct RenameState {
    id: Uuid,
    title: String,
    request_focus: bool,
    error: Option<&'static str>,
}

impl BlockApp {
    fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let data_dir = eframe::storage_dir(APP_ID)
            .ok_or_else(|| io::Error::other("application-data directory is unavailable"))?
            .join("blocks");
        let url = start_embedded_server(data_dir)?;
        let client = BlockClient::new();
        let workspace = client.get_or_create_block(WORKSPACE_INDEX_ID, WorkspaceIndex::default());
        client.connect(url);

        Ok(Self {
            client,
            workspace,
            registry: EditorRegistry::new(),
            editors: HashMap::new(),
            tabs: Vec::new(),
            active_tab: None,
            rename: None,
        })
    }

    fn entries(&self) -> Vec<BlockEntry> {
        self.workspace
            .read()
            .map(|index| index.entries().to_vec())
            .unwrap_or_default()
    }

    fn create_text_block(&mut self) {
        let Some(editor) = self.registry.create(&self.client, TextDocument::TYPE_ID) else {
            return;
        };
        let id = editor.id();
        self.editors.insert(id, editor);
        self.workspace
            .operate(WorkspaceIndexOperation::Add(BlockEntry {
                id,
                block_type: TextDocument::TYPE_ID,
                title: DEFAULT_TITLE.into(),
            }));
        self.open_tab(id, TextDocument::TYPE_ID);
        self.rename = Some(RenameState {
            id,
            title: DEFAULT_TITLE.into(),
            request_focus: true,
            error: None,
        });
    }

    fn open_tab(&mut self, id: Uuid, block_type: Uuid) {
        if !self.editors.contains_key(&id) {
            let editor = self.registry.open(&self.client, id, block_type);
            self.editors.insert(id, editor);
        }
        if !self.tabs.contains(&id) {
            self.tabs.push(id);
        }
        self.active_tab = Some(id);
    }

    fn close_tab(&mut self, id: Uuid) {
        let Some(index) = self.tabs.iter().position(|open| *open == id) else {
            return;
        };
        self.tabs.remove(index);
        if self.active_tab == Some(id) {
            self.active_tab = if self.tabs.is_empty() {
                None
            } else {
                Some(self.tabs[index.min(self.tabs.len() - 1)])
            };
        }
    }

    fn begin_rename(&mut self, entry: &BlockEntry) {
        self.rename = Some(RenameState {
            id: entry.id,
            title: entry.title.clone(),
            request_focus: true,
            error: None,
        });
    }

    fn commit_rename(&mut self) -> bool {
        let Some(rename) = &mut self.rename else {
            return true;
        };
        let title = rename.title.trim();
        if title.is_empty() {
            rename.error = Some("Title cannot be empty");
            return false;
        }
        self.workspace.operate(WorkspaceIndexOperation::Rename {
            id: rename.id,
            title: title.to_owned(),
        });
        self.rename = None;
        true
    }

    fn show_sidebar(&mut self, ui: &mut egui::Ui, entries: &[BlockEntry]) {
        ui.horizontal(|ui| {
            ui.heading("Blocks");
            ui.add_space(ui.available_width() - 28.0);
            if ui.button("+").on_hover_text("Create text block").clicked() {
                self.create_text_block();
            }
        });
        ui.separator();

        let mut open = None;
        let mut begin_rename = None;
        let mut commit_rename = false;
        let mut cancel_rename = false;

        egui::ScrollArea::vertical().show(ui, |ui| {
            for entry in entries {
                if self
                    .rename
                    .as_ref()
                    .is_some_and(|rename| rename.id == entry.id)
                {
                    let rename = self.rename.as_mut().unwrap();
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut rename.title).desired_width(f32::INFINITY),
                    );
                    if rename.request_focus {
                        response.request_focus();
                        rename.request_focus = false;
                    }
                    if response.changed() {
                        rename.error = None;
                    }
                    if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                        cancel_rename = true;
                    } else if ui.input(|input| input.key_pressed(egui::Key::Enter))
                        || response.lost_focus()
                    {
                        commit_rename = true;
                    }
                    if let Some(error) = rename.error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                    continue;
                }

                ui.horizontal(|ui| {
                    let selected = self.active_tab == Some(entry.id);
                    let response =
                        ui.selectable_label(selected, &entry.title)
                            .on_hover_text(format!(
                                "{}\n{}",
                                self.registry
                                    .display_name(entry.block_type)
                                    .unwrap_or("Unsupported"),
                                entry.id
                            ));
                    if response.clicked() {
                        open = Some((entry.id, entry.block_type));
                    }
                    response.context_menu(|ui| {
                        if ui.button("Rename").clicked() {
                            begin_rename = Some(entry.clone());
                            ui.close();
                        }
                    });
                    if ui.small_button("…").clicked() {
                        begin_rename = Some(entry.clone());
                    }
                });
            }
        });

        if cancel_rename {
            self.rename = None;
        } else if commit_rename {
            self.commit_rename();
        }
        if let Some(entry) = begin_rename {
            self.begin_rename(&entry);
        }
        if let Some((id, block_type)) = open {
            self.open_tab(id, block_type);
        }
    }

    fn show_tabs(&mut self, ui: &mut egui::Ui, entries: &[BlockEntry]) {
        let titles: HashMap<_, _> = entries
            .iter()
            .map(|entry| (entry.id, entry.title.as_str()))
            .collect();
        let mut activate = None;
        let mut close = None;

        egui::ScrollArea::horizontal()
            .id_salt("block-tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for id in &self.tabs {
                        let title = titles.get(id).copied().unwrap_or("Unknown block");
                        let active = self.active_tab == Some(*id);
                        egui::Frame::new()
                            .fill(if active {
                                ui.visuals().extreme_bg_color
                            } else {
                                ui.visuals().faint_bg_color
                            })
                            .inner_margin(egui::Margin::symmetric(8, 4))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if ui.selectable_label(active, title).clicked() {
                                        activate = Some(*id);
                                    }
                                    if ui.small_button("×").clicked() {
                                        close = Some(*id);
                                    }
                                });
                            });
                    }
                });
            });

        if let Some(id) = activate {
            self.active_tab = Some(id);
        }
        if let Some(id) = close {
            self.close_tab(id);
        }
    }

    fn show_content(&mut self, ui: &mut egui::Ui) {
        let Some(active) = self.active_tab else {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("No block open");
                    ui.weak("Create or select a block from the sidebar.");
                });
            });
            return;
        };
        let Some(editor) = self.editors.get_mut(&active) else {
            self.active_tab = None;
            return;
        };
        editor.ui(ui);
    }
}

impl eframe::App for BlockApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let entries = self.entries();
        egui::Panel::left("blocks-sidebar")
            .default_size(240.0)
            .min_size(160.0)
            .max_size(420.0)
            .resizable(true)
            .show_inside(ui, |ui| self.show_sidebar(ui, &entries));

        ui.vertical(|ui| {
            self.show_tabs(ui, &entries);
            ui.separator();
            self.show_content(ui);
        });
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
}

fn start_embedded_server(data_dir: PathBuf) -> Result<String, Box<dyn Error + Send + Sync>> {
    let listener = StdTcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    thread::Builder::new()
        .name("block-app-server".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to create embedded block server runtime");
            runtime.block_on(async move {
                let listener = TcpListener::from_std(listener)
                    .expect("failed to initialize embedded block server listener");
                if let Err(error) = block_server::serve(listener, data_dir).await {
                    eprintln!("embedded block server stopped: {error}");
                }
            });
        })?;
    Ok(format!("ws://{address}"))
}
