mod block_picker;
mod debug;
mod editors;

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    io,
    net::TcpListener as StdTcpListener,
    path::PathBuf,
    thread,
    time::Duration,
};

use block::{Block, BlockParent, BlockReference, BlockReferenceList, MAX_NAME_BYTES};
use block_client::{
    blocks::workspace_index::{BlockEntry, WorkspaceIndex},
    BlockClient, ReferenceList,
};
use block_picker::{BlockPicker, BlockPickerMenuAction};
use debug::browser::BrowserDebug;
use editors::{BlockEditor, EditorAction, EditorRegistry};
use eframe::egui;
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
        Box::new(|creation_context| {
            BlockApp::new(creation_context)
                .map(|app| Box::new(app) as Box<dyn eframe::App>)
                .map_err(Into::into)
        }),
    )
}

struct BlockApp {
    client: BlockClient,
    roots: ReferenceList,
    orphaned: Option<ReferenceList>,
    orphaned_expanded: bool,
    expanded: HashMap<Uuid, ReferenceList>,
    registry: EditorRegistry,
    editors: HashMap<Uuid, Box<dyn BlockEditor>>,
    tabs: Vec<Uuid>,
    active_tab: Option<Uuid>,
    pending_placements: Vec<PendingPlacement>,
    rename: Option<RenameState>,
    network_debug_open: bool,
    browser_debug: BrowserDebug,
    block_picker: BlockPicker,
    block_picker_target: Option<BlockPickerTarget>,
}

#[derive(Clone)]
struct PendingPlacement {
    child: BlockReference,
    parent: Uuid,
}

struct RenameState {
    id: Uuid,
    name: String,
}

#[derive(Clone, Copy)]
enum BlockPickerTarget {
    Root,
    Folder(Uuid),
}

impl BlockApp {
    fn new(
        creation_context: &eframe::CreationContext<'_>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let data_dir = eframe::storage_dir(APP_ID)
            .ok_or_else(|| io::Error::other("application-data directory is unavailable"))?
            .join("blocks");
        let url = start_embedded_server(data_dir)?;
        let client = BlockClient::new();
        client.connect(url);
        let roots = client.watch_references(BlockReferenceList::Roots);
        let browser_debug = BrowserDebug::new(creation_context)?;

        Ok(Self {
            client,
            roots,
            orphaned: None,
            orphaned_expanded: false,
            expanded: HashMap::new(),
            registry: EditorRegistry::new(),
            editors: HashMap::new(),
            tabs: Vec::new(),
            active_tab: None,
            pending_placements: Vec::new(),
            rename: None,
            network_debug_open: false,
            browser_debug,
            block_picker: BlockPicker::default(),
            block_picker_target: None,
        })
    }

    fn create_block_editor(&mut self, block_type: Uuid) -> Option<Uuid> {
        let Some(editor) = self.registry.create(&self.client, block_type) else {
            return None;
        };
        let id = editor.id();
        self.editors.insert(id, editor);
        Some(id)
    }

    fn create_block(&mut self, block_type: Uuid, parent: Option<Uuid>) {
        let Some(id) = self.create_block_editor(block_type) else {
            return;
        };
        if let Some(parent) = parent {
            self.queue_placement(
                BlockReference {
                    id,
                    block_type,
                    name: if block_type == WorkspaceIndex::TYPE_ID {
                        "Folder".into()
                    } else {
                        String::new()
                    },
                    parent: BlockParent::Root,
                    references: 0,
                },
                parent,
            );
        }
        self.open_tab(id, block_type);
    }

    fn link_cached_block(&mut self, block: block_client::CachedBlock, target: BlockPickerTarget) {
        match target {
            BlockPickerTarget::Root => {
                if !self.editors.contains_key(&block.id) {
                    self.editors.insert(
                        block.id,
                        self.registry.open(&self.client, block.id, block.block_type),
                    );
                }
                self.editors[&block.id].set_parent(BlockParent::Root);
            }
            BlockPickerTarget::Folder(parent) => self.queue_placement(
                BlockReference {
                    id: block.id,
                    block_type: block.block_type,
                    name: block.name,
                    parent: BlockParent::Root,
                    references: 0,
                },
                parent,
            ),
        }
    }

    fn handle_picker_menu_action(
        &mut self,
        action: BlockPickerMenuAction,
        target: BlockPickerTarget,
        excluded: impl IntoIterator<Item = Uuid>,
    ) {
        match action {
            BlockPickerMenuAction::New(block_type) => {
                let parent = match target {
                    BlockPickerTarget::Root => None,
                    BlockPickerTarget::Folder(parent) => Some(parent),
                };
                self.create_block(block_type, parent);
            }
            BlockPickerMenuAction::LinkExisting => {
                self.block_picker.open(excluded);
                self.block_picker_target = Some(target);
            }
        }
    }

    fn show_block_picker(&mut self, context: &egui::Context) {
        let Some(block) = self.block_picker.show(context, &self.client) else {
            return;
        };
        if let Some(target) = self.block_picker_target.take() {
            self.link_cached_block(block, target);
        }
    }

    fn queue_placement(&mut self, child: BlockReference, parent: Uuid) {
        if child.id == parent
            || self
                .pending_placements
                .iter()
                .any(|pending| pending.child.id == child.id && pending.parent == parent)
        {
            return;
        }
        if !self.editors.contains_key(&child.id) {
            self.editors.insert(
                child.id,
                self.registry.open(&self.client, child.id, child.block_type),
            );
        }
        if !self.editors.contains_key(&parent) {
            self.editors.insert(
                parent,
                self.registry
                    .open(&self.client, parent, WorkspaceIndex::TYPE_ID),
            );
        }
        self.pending_placements
            .push(PendingPlacement { child, parent });
    }

    fn process_pending_placements(&mut self) {
        let pending = std::mem::take(&mut self.pending_placements);
        for placement in pending {
            let entry = BlockEntry {
                id: placement.child.id,
            };
            let ready = self
                .editors
                .get(&placement.parent)
                .and_then(|editor| editor.add_child(entry));
            if ready == Some(true) {
                if let Some(child) = self.editors.get(&placement.child.id) {
                    child.note_backref(placement.parent);
                    child.set_parent(BlockParent::Uuid(placement.parent));
                }
            } else {
                self.pending_placements.push(placement);
            }
        }
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

    fn reference_label(&self, reference: &BlockReference) -> String {
        reference.name.clone()
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
        let mut picker_action = None;
        let mut row_response = None;
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 14.0);
            if ui
                .small_button(if was_expanded { "▼" } else { "▶" })
                .clicked()
            {
                toggle = true;
            }
            let trailing_width = if reference.block_type == WorkspaceIndex::TYPE_ID {
                ui.spacing().interact_size.x + ui.spacing().item_spacing.x
            } else {
                0.0
            };
            let label_width = (ui.available_width() - trailing_width).max(0.0);
            let response = ui
                .add_sized(
                    [label_width, ui.spacing().interact_size.y],
                    egui::Button::selectable(
                        self.active_tab == Some(reference.id),
                        self.reference_label(&reference),
                    )
                    .right_text(())
                    .truncate()
                    .sense(egui::Sense::click_and_drag()),
                )
                .on_hover_text(reference.id.to_string());
            response.dnd_set_drag_payload(reference.clone());
            if response.clicked() {
                open = true;
            }
            response.context_menu(|ui| {
                ui.add_enabled_ui(reference.block_type == WorkspaceIndex::TYPE_ID, |ui| {
                    ui.menu_button("Add", |ui| {
                        picker_action = BlockPicker::show_menu(ui);
                    });
                });
                if ui.button("Rename").clicked() {
                    self.rename = Some(RenameState {
                        id: reference.id,
                        name: reference.name.clone(),
                    });
                    ui.close();
                }
            });
            row_response = Some(response);
            if reference.block_type == WorkspaceIndex::TYPE_ID {
                ui.menu_button("+", |ui| {
                    picker_action = BlockPicker::show_menu(ui);
                })
                .response
                .on_hover_text("Create inside this folder");
            }
        });

        if let Some(action) = picker_action {
            let mut excluded = path.clone();
            excluded.insert(reference.id);
            self.handle_picker_menu_action(
                action,
                BlockPickerTarget::Folder(reference.id),
                excluded,
            );
        }
        if reference.block_type == WorkspaceIndex::TYPE_ID {
            if let Some(response) = row_response {
                if let Some(dragged) = response.dnd_hover_payload::<BlockReference>() {
                    let valid = dragged.id != reference.id && !path.contains(&dragged.id);
                    let color = if valid {
                        ui.visuals().selection.stroke.color
                    } else {
                        ui.visuals().error_fg_color
                    };
                    ui.painter().rect_stroke(
                        response.rect,
                        3.0,
                        egui::Stroke::new(1.0, color),
                        egui::StrokeKind::Outside,
                    );
                }
                if let Some(dragged) = response.dnd_release_payload::<BlockReference>() {
                    if dragged.id != reference.id && !path.contains(&dragged.id) {
                        self.queue_placement(dragged.as_ref().clone(), reference.id);
                    }
                }
            }
        }

        if toggle {
            if was_expanded {
                self.collapse_reference(reference.id);
            } else {
                self.expanded.insert(
                    reference.id,
                    self.client
                        .watch_references(BlockReferenceList::References(reference.id)),
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
        let mut picker_action = None;
        ui.horizontal(|ui| {
            ui.heading("Blocks");
            ui.add_space(ui.available_width() - 28.0);
            ui.menu_button("+", |ui| {
                picker_action = BlockPicker::show_menu(ui);
            })
            .response
            .on_hover_text("Create block");
        });
        if let Some(action) = picker_action {
            self.handle_picker_menu_action(action, BlockPickerTarget::Root, []);
        }
        ui.separator();

        egui::Panel::bottom("file-list-status")
            .resizable(false)
            .show_inside(ui, |ui| self.show_file_list_status(ui));

        egui::ScrollArea::vertical().show(ui, |ui| {
            let roots = self.roots.read();
            if roots.is_empty() && self.roots.is_loaded() {
                ui.weak("No root blocks");
            }
            for root in roots {
                self.show_reference(ui, root, 0, &mut HashSet::new());
            }
            ui.separator();
            self.show_recently_deleted(ui);
        });
    }

    fn show_recently_deleted(&mut self, ui: &mut egui::Ui) {
        let mut toggle = false;
        ui.horizontal(|ui| {
            if ui
                .small_button(if self.orphaned_expanded {
                    "\u{25bc}"
                } else {
                    "\u{25b6}"
                })
                .clicked()
            {
                toggle = true;
            }
            if ui
                .selectable_label(false, "Recently Deleted")
                .on_hover_text("Blocks that no longer have a parent")
                .clicked()
            {
                toggle = true;
            }
        });

        if toggle {
            self.orphaned_expanded = !self.orphaned_expanded;
            self.orphaned = self
                .orphaned_expanded
                .then(|| self.client.watch_references(BlockReferenceList::Orphans));
        }

        if !self.orphaned_expanded {
            return;
        }
        let Some(orphaned) = &self.orphaned else {
            return;
        };
        let blocks = orphaned.read();
        let loaded = orphaned.is_loaded();
        if blocks.is_empty() && loaded {
            ui.horizontal(|ui| {
                ui.add_space(36.0);
                ui.weak("No recently deleted blocks");
            });
        }
        for block in blocks {
            self.show_reference(ui, block, 1, &mut HashSet::new());
        }
    }

    fn show_file_list_status(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        let debug = self.client.network_debug_snapshot();
        ui.horizontal(|ui| {
            if debug.changes_saved {
                ui.small("\u{2713} All changes saved");
            } else {
                ui.spinner();
                ui.small("Submitting changes\u{2026}");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("Debug", |ui| {
                    if ui.button("Browser").clicked() {
                        self.browser_debug.open();
                        ui.close();
                    }
                    if ui.button("Network").clicked() {
                        self.network_debug_open = true;
                        ui.close();
                    }
                });
            });
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
                                    let label = self
                                        .editors
                                        .get(id)
                                        .map_or_else(|| id.to_string(), |editor| editor.name());
                                    if ui.selectable_label(active, label).clicked() {
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
        let action = editor.ui(ui, &self.client);
        match action {
            Some(EditorAction::OpenBlock { id, block_type }) => self.open_tab(id, block_type),
            Some(EditorAction::CreateBlock { block_type, parent }) => {
                if let Some(id) = self.create_block_editor(block_type) {
                    if let Some(parent) = parent {
                        if let Some(created) = self.editors.get(&id) {
                            created.set_parent(BlockParent::Uuid(parent));
                            created.note_backref(parent);
                        }
                    }
                    let name = self
                        .registry
                        .display_name(block_type)
                        .unwrap_or_default()
                        .to_owned();
                    if let Some(editor) = self.editors.get_mut(&active) {
                        editor.block_created(id, block_type, name);
                    }
                }
            }
            None => {}
        }
    }

    fn show_statusbar(&mut self, ui: &mut egui::Ui) {
        let Some(active) = self.active_tab else {
            return;
        };
        let Some(editor) = self.editors.get(&active) else {
            return;
        };
        let block_type = editor.block_type();
        let type_name = self
            .registry
            .display_name(block_type)
            .map_or_else(|| block_type.to_string(), str::to_owned);
        let relationships = editor.relationships();
        let mut new_parent = None;

        ui.horizontal_wrapped(|ui| {
            ui.small(format!("Type: {type_name}"));
            ui.separator();
            let Some(relationships) = &relationships else {
                ui.small("Relationships loading…");
                return;
            };

            ui.menu_button(
                format!("Parent: {}", parent_label(relationships.parent)),
                |ui| {
                    if relationships.backrefs.is_empty() {
                        ui.weak("No backrefs available");
                    }
                    for backref in &relationships.backrefs {
                        if ui
                            .selectable_label(
                                relationships.parent == BlockParent::Uuid(*backref),
                                backref.to_string(),
                            )
                            .clicked()
                        {
                            new_parent = Some(BlockParent::Uuid(*backref));
                            ui.close();
                        }
                    }
                },
            );
            ui.separator();
            ui.menu_button(
                format!("Backrefs: {}", relationships.backrefs.len()),
                |ui| immutable_id_list(ui, &relationships.backrefs, "No backrefs"),
            );
            ui.separator();
            ui.menu_button(
                format!("References: {}", relationships.references.len()),
                |ui| immutable_id_list(ui, &relationships.references, "No references"),
            );
        });

        if let Some(parent) = new_parent {
            if let Some(editor) = self.editors.get(&active) {
                editor.set_parent(parent);
            }
        }
    }

    fn show_rename(&mut self, ui: &mut egui::Ui) {
        let Some(rename) = &mut self.rename else {
            return;
        };
        let mut submit = false;
        let mut cancel = false;
        egui::Window::new("Rename block")
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                let response = ui.text_edit_singleline(&mut rename.name);
                let valid = rename.name.len() <= MAX_NAME_BYTES;
                if !valid {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("Name must be at most {MAX_NAME_BYTES} UTF-8 bytes."),
                    );
                }
                ui.horizontal(|ui| {
                    submit = ui.add_enabled(valid, egui::Button::new("Rename")).clicked()
                        || (valid
                            && response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if submit {
            let rename = self.rename.take().unwrap();
            self.client.set_block_name(rename.id, rename.name);
        } else if cancel {
            self.rename = None;
        }
    }
}

fn parent_label(parent: BlockParent) -> String {
    match parent {
        BlockParent::Root => "Root".into(),
        BlockParent::Orphaned => "Orphaned".into(),
        BlockParent::Uuid(id) => id.to_string(),
    }
}

fn immutable_id_list(ui: &mut egui::Ui, ids: &[Uuid], empty: &str) {
    if ids.is_empty() {
        ui.weak(empty);
    }
    for id in ids {
        ui.label(id.to_string());
    }
}

impl eframe::App for BlockApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.process_pending_placements();
        self.show_block_picker(ui.ctx());
        self.show_rename(ui);
        self.show_network_debug(ui.ctx());
        self.browser_debug.show(ui.ctx());
        egui::Panel::left("blocks-sidebar")
            .default_size(240.0)
            .min_size(160.0)
            .max_size(420.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                let content_rect = ui.available_rect_before_wrap();
                let mut content_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("blocks-sidebar-content")
                        .max_rect(content_rect),
                );
                content_ui.set_clip_rect(content_rect.intersect(ui.clip_rect()));
                content_ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                self.show_sidebar(&mut content_ui);
                ui.advance_cursor_after_rect(content_rect);
            });

        ui.vertical(|ui| {
            self.show_tabs(ui);
            ui.separator();
            egui::Panel::bottom("block-statusbar")
                .resizable(false)
                .show_inside(ui, |ui| self.show_statusbar(ui));
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
