mod editor;
mod index;

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    io,
    net::TcpListener as StdTcpListener,
    path::PathBuf,
    thread,
    time::Duration,
};

use block::{Block, BlockParent, BlockReference};
use block_client::{text::TextDocument, BlockClient, ReferenceList};
use editor::{BlockEditor, EditorRegistry};
use eframe::egui;
use index::WorkspaceIndex;
use tokio::net::TcpListener;
use uuid::Uuid;

const APP_ID: &str = "Block";

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
    roots: ReferenceList,
    expanded: HashMap<Uuid, ReferenceList>,
    registry: EditorRegistry,
    editors: HashMap<Uuid, Box<dyn BlockEditor>>,
    tabs: Vec<Uuid>,
    active_tab: Option<Uuid>,
}

impl BlockApp {
    fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let data_dir = eframe::storage_dir(APP_ID)
            .ok_or_else(|| io::Error::other("application-data directory is unavailable"))?
            .join("blocks");
        let url = start_embedded_server(data_dir)?;
        let client = BlockClient::new();
        client.connect(url);
        let roots = client.watch_references(BlockParent::Root);

        Ok(Self {
            client,
            roots,
            expanded: HashMap::new(),
            registry: EditorRegistry::new(),
            editors: HashMap::new(),
            tabs: Vec::new(),
            active_tab: None,
        })
    }

    fn create_block(&mut self, block_type: Uuid) {
        let Some(editor) = self.registry.create(&self.client, block_type) else {
            return;
        };
        let id = editor.id();
        self.editors.insert(id, editor);
        self.open_tab(id, block_type);
    }

    fn open_tab(&mut self, id: Uuid, block_type: Uuid) {
        if !self.editors.contains_key(&id) {
            self.editors
                .insert(id, self.registry.open(&self.client, id, block_type));
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

    fn reference_label(&self, reference: BlockReference) -> String {
        let kind = self
            .registry
            .display_name(reference.block_type)
            .unwrap_or("Unsupported");
        format!("{kind} · {}", &reference.id.to_string()[..8])
    }

    fn collapse_reference(&mut self, id: Uuid) {
        self.collapse_reference_inner(id, &mut HashSet::new());
    }

    fn collapse_reference_inner(&mut self, id: Uuid, visited: &mut HashSet<Uuid>) {
        if !visited.insert(id) {
            return;
        }
        let children = self
            .expanded
            .remove(&id)
            .map(|list| list.read())
            .unwrap_or_default();
        for child in children {
            self.collapse_reference_inner(child.id, visited);
        }
    }

    fn show_reference(
        &mut self,
        ui: &mut egui::Ui,
        reference: BlockReference,
        depth: usize,
        path: &mut HashSet<Uuid>,
    ) {
        let was_expanded = self.expanded.contains_key(&reference.id);
        let mut toggle = false;
        let mut open = false;
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 14.0);
            if ui
                .small_button(if was_expanded { "▼" } else { "▶" })
                .clicked()
            {
                toggle = true;
            }
            if ui
                .selectable_label(
                    self.active_tab == Some(reference.id),
                    self.reference_label(reference),
                )
                .on_hover_text(reference.id.to_string())
                .clicked()
            {
                open = true;
            }
        });

        if toggle {
            if was_expanded {
                self.collapse_reference(reference.id);
            } else {
                self.expanded.insert(
                    reference.id,
                    self.client
                        .watch_references(BlockParent::Uuid(reference.id)),
                );
            }
        }
        if open {
            self.open_tab(reference.id, reference.block_type);
        }

        let is_expanded = self.expanded.contains_key(&reference.id);
        if !is_expanded || !path.insert(reference.id) {
            return;
        }
        let children = self.expanded[&reference.id].read();
        if children.is_empty() && self.expanded[&reference.id].is_loaded() {
            ui.horizontal(|ui| {
                ui.add_space((depth + 1) as f32 * 14.0 + 22.0);
                ui.weak("No references");
            });
        }
        for child in children {
            self.show_reference(ui, child, depth + 1, path);
        }
        path.remove(&reference.id);
    }

    fn show_sidebar(&mut self, ui: &mut egui::Ui) {
        let mut create = None;
        ui.horizontal(|ui| {
            ui.heading("Blocks");
            ui.add_space(ui.available_width() - 28.0);
            ui.menu_button("+", |ui| {
                if ui.button("Text block").clicked() {
                    create = Some(TextDocument::TYPE_ID);
                    ui.close();
                }
                if ui.button("Folder").clicked() {
                    create = Some(WorkspaceIndex::TYPE_ID);
                    ui.close();
                }
            })
            .response
            .on_hover_text("Create block");
        });
        if let Some(block_type) = create {
            self.create_block(block_type);
        }
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let roots = self.roots.read();
            if roots.is_empty() && self.roots.is_loaded() {
                ui.weak("No root blocks");
            }
            for root in roots {
                self.show_reference(ui, root, 0, &mut HashSet::new());
            }
        });
    }

    fn show_tabs(&mut self, ui: &mut egui::Ui) {
        let mut activate = None;
        let mut close = None;
        egui::ScrollArea::horizontal()
            .id_salt("block-tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for id in &self.tabs {
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
                                    if ui.selectable_label(active, &id.to_string()[..8]).clicked() {
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
        egui::Panel::left("blocks-sidebar")
            .default_size(240.0)
            .min_size(160.0)
            .max_size(420.0)
            .resizable(true)
            .show_inside(ui, |ui| self.show_sidebar(ui));

        ui.vertical(|ui| {
            self.show_tabs(ui);
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
