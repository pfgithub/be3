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

use block::{BlockParent, BlockReference, BlockReferenceList, MAX_NAME_BYTES};
use block_client::{blocks::workspace_index::BlockEntry, BlockClient, ReferenceList};
use block_picker::{BlockPicker, BlockPickerMenuAction};
use editors::{
    BlockEditor, DynamicArtifactRegeneration, EditorAccess, EditorAction, EditorRegistry,
    SidebarDragPayload, SidebarDragSource,
};
use eframe::egui;
use egui_dock::{widgets::tab_viewer::OnCloseResponse, DockArea, DockState, TabViewer};
use tokio::net::TcpListener;
use uuid::Uuid;

const APP_ID: &str = "Block";
const ACCOUNTS: [Account; 5] = [
    Account {
        name: "Account A",
        id: Uuid::from_u128(0x88ee0604_544d_4b45_bfed_75b295301153),
    },
    Account {
        name: "Account B",
        id: Uuid::from_u128(0x380531b6_3b86_452d_bbcd_ae94ce002f8e),
    },
    Account {
        name: "Account C",
        id: Uuid::from_u128(0x81a47f30_6c99_4154_8075_5d53998d6cd4),
    },
    Account {
        name: "Account D",
        id: Uuid::from_u128(0xc4b6705d_f2be_403e_a1b7_bc336314d741),
    },
    Account {
        name: "Account E",
        id: Uuid::from_u128(0x51db8e56_fe52_42c7_9d92_1c298502fa32),
    },
];

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
        Box::new(|_creation_context| {
            BlockApp::new()
                .map(|app| Box::new(app) as Box<dyn eframe::App>)
                .map_err(Into::into)
        }),
    )
}

struct BlockApp {
    server_url: String,
    account: Account,
    client: BlockClient,
    roots: ReferenceList,
    orphaned: Option<ReferenceList>,
    orphaned_expanded: bool,
    expanded: HashMap<Uuid, ReferenceList>,
    parents: HashMap<Uuid, ReferenceList>,
    references: HashMap<Uuid, ReferenceList>,
    backrefs: HashMap<Uuid, ReferenceList>,
    block_types: HashMap<Uuid, Uuid>,
    registry: EditorRegistry,
    editors: HashMap<Uuid, Box<dyn BlockEditor>>,
    dynamic_artifact_regenerations: HashMap<Uuid, Box<dyn DynamicArtifactRegeneration>>,
    dynamic_artifact_errors: HashMap<Uuid, String>,
    dock_state: DockState<DockTab>,
    active_tab: Option<Uuid>,
    sidebar_reveal: Option<Uuid>,
    pending_transfers: Vec<PendingTransfer>,
    rename: Option<RenameState>,
    client_debug_open: bool,
    network_debug_open: bool,
    block_picker: BlockPicker,
    block_picker_target: Option<BlockPickerTarget>,
    pending_destructive_action: Option<PendingDestructiveAction>,
    scheduled_account_switch: Option<Account>,
    allow_close: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum DockTab {
    Files,
    Empty,
    Block(Uuid),
}

fn default_dock_state() -> DockState<DockTab> {
    let mut dock_state = DockState::new(vec![DockTab::Files]);
    let files_path = dock_state
        .find_tab(&DockTab::Files)
        .expect("new dock state must contain Files");
    dock_state[files_path.surface].split_right(files_path.node, 0.22, vec![DockTab::Empty]);
    dock_state
}

fn ensure_empty_workspace(dock_state: &mut DockState<DockTab>) {
    let has_editor = dock_state
        .iter_all_tabs()
        .any(|(_, tab)| matches!(tab, DockTab::Block(_)));
    if has_editor || dock_state.find_tab(&DockTab::Empty).is_some() {
        return;
    }
    if let Some(files_path) = dock_state.find_tab(&DockTab::Files) {
        dock_state[files_path.surface].split_right(files_path.node, 0.22, vec![DockTab::Empty]);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Account {
    name: &'static str,
    id: Uuid,
}

#[derive(Clone, Copy)]
enum PendingDestructiveAction {
    Switch(Account),
    Close,
}

#[derive(Clone)]
struct PendingTransfer {
    child: BlockReference,
    source: Option<SidebarDragSource>,
    destination: Option<Uuid>,
    parent_after: Option<BlockParent>,
    stage: TransferStage,
}

#[derive(Clone, Copy)]
enum TransferStage {
    DeleteSource,
    AddDestination,
}

struct RenameState {
    id: Uuid,
    name: String,
}

#[derive(Clone)]
struct SidebarActiveLocation {
    id: Uuid,
    name: String,
    root: BlockParent,
    path: Vec<Uuid>,
}

struct SidebarHighlight {
    rect: egui::Rect,
    collapsed: bool,
}

#[derive(Default)]
struct SidebarRenderState {
    highlight: Option<SidebarHighlight>,
    scroll_requested: bool,
}

#[derive(Clone, Copy)]
enum BlockPickerTarget {
    Root,
    Block(Uuid),
}

impl BlockApp {
    fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let data_dir = eframe::storage_dir(APP_ID)
            .ok_or_else(|| io::Error::other("application-data directory is unavailable"))?
            .join("blocks");
        let url = start_embedded_server(data_dir)?;
        let account = ACCOUNTS[0];
        let client = BlockClient::new(account.id);
        client.connect(url.clone());
        let roots = client.watch_references(BlockReferenceList::Roots);
        Ok(Self {
            server_url: url,
            account,
            client,
            roots,
            orphaned: None,
            orphaned_expanded: false,
            expanded: HashMap::new(),
            parents: HashMap::new(),
            references: HashMap::new(),
            backrefs: HashMap::new(),
            block_types: HashMap::new(),
            registry: EditorRegistry::new(),
            editors: HashMap::new(),
            dynamic_artifact_regenerations: HashMap::new(),
            dynamic_artifact_errors: HashMap::new(),
            dock_state: default_dock_state(),
            active_tab: None,
            sidebar_reveal: None,
            pending_transfers: Vec::new(),
            rename: None,
            client_debug_open: false,
            network_debug_open: false,
            block_picker: BlockPicker::default(),
            block_picker_target: None,
            pending_destructive_action: None,
            scheduled_account_switch: None,
            allow_close: false,
        })
    }

    fn request_account_switch(&mut self, account: Account) {
        if account == self.account {
            return;
        }
        if self.client.network_debug_snapshot().changes_saved {
            self.scheduled_account_switch = Some(account);
        } else {
            self.pending_destructive_action = Some(PendingDestructiveAction::Switch(account));
        }
    }

    fn switch_account(&mut self, ctx: &egui::Context, account: Account) {
        let client = BlockClient::new(account.id);
        client.connect(self.server_url.clone());
        let roots = client.watch_references(BlockReferenceList::Roots);

        self.orphaned = None;
        self.orphaned_expanded = false;
        self.expanded.clear();
        self.parents.clear();
        self.references.clear();
        self.backrefs.clear();
        self.block_types.clear();
        self.registry = EditorRegistry::new();
        self.editors.clear();
        self.dynamic_artifact_regenerations.clear();
        self.dynamic_artifact_errors.clear();
        self.dock_state = default_dock_state();
        self.active_tab = None;
        self.sidebar_reveal = None;
        self.pending_transfers.clear();
        self.rename = None;
        self.client_debug_open = false;
        self.network_debug_open = false;
        self.block_picker = BlockPicker::default();
        self.block_picker_target = None;
        self.pending_destructive_action = None;
        self.scheduled_account_switch = None;
        self.allow_close = false;
        self.roots = roots;
        self.client = client;
        self.account = account;
        ctx.memory_mut(|memory| *memory = Default::default());
    }

    fn intercept_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|input| input.viewport().close_requested()) || self.allow_close {
            return;
        }
        if self.client.network_debug_snapshot().changes_saved {
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.pending_destructive_action = Some(PendingDestructiveAction::Close);
    }

    fn show_discard_confirmation(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending_destructive_action else {
            return;
        };
        let mut discard = false;
        let mut cancel = false;
        let (title, message, button) = match action {
            PendingDestructiveAction::Switch(account) => (
                "Discard unsaved changes?",
                format!(
                    "Switching to {} will discard changes that have not reached the server.",
                    account.name
                ),
                "Discard and switch",
            ),
            PendingDestructiveAction::Close => (
                "Discard unsaved changes?",
                "Closing Block Editor will discard changes that have not reached the server."
                    .into(),
                "Discard and close",
            ),
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(message);
                ui.horizontal(|ui| {
                    discard = ui.button(button).clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if discard {
            self.pending_destructive_action = None;
            match action {
                PendingDestructiveAction::Switch(account) => {
                    self.scheduled_account_switch = Some(account);
                }
                PendingDestructiveAction::Close => {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        } else if cancel {
            self.pending_destructive_action = None;
        }
    }

    fn create_block_editor(&mut self, block_type: Uuid) -> Option<Uuid> {
        let Some(editor) = self.registry.create(&self.client, block_type) else {
            return None;
        };
        let id = editor.id();
        self.block_types.insert(id, block_type);
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
                    author: self.account.id,
                    name: self
                        .registry
                        .display_name(block_type)
                        .unwrap_or_default()
                        .into(),
                    parent: BlockParent::Root,
                    references: 0,
                },
                parent,
            );
        } else if let Some(editor) = self.editors.get(&id) {
            editor.set_parent(BlockParent::Root);
        }
        self.open_tab(id, block_type);
    }

    fn link_cached_block(&mut self, block: block_client::CachedBlock, target: BlockPickerTarget) {
        match target {
            BlockPickerTarget::Root => {
                self.block_types.insert(block.id, block.block_type);
                if !self.editors.contains_key(&block.id) {
                    self.editors.insert(
                        block.id,
                        self.registry.open(&self.client, block.id, block.block_type),
                    );
                }
                self.editors[&block.id].set_parent(BlockParent::Root);
            }
            BlockPickerTarget::Block(parent) => self.queue_placement(
                BlockReference {
                    id: block.id,
                    block_type: block.block_type,
                    author: block.author,
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
                    BlockPickerTarget::Block(parent) => Some(parent),
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

    fn ensure_editor(&mut self, id: Uuid) -> bool {
        if self.editors.contains_key(&id) {
            return true;
        }
        let Some(block_type) = self.block_types.get(&id).copied() else {
            return false;
        };
        self.editors
            .insert(id, self.registry.open(&self.client, id, block_type));
        true
    }

    fn queue_placement(&mut self, child: BlockReference, parent: Uuid) {
        if child.id == parent
            || self
                .pending_transfers
                .iter()
                .any(|pending| pending.child.id == child.id && pending.destination == Some(parent))
        {
            return;
        }
        self.block_types.insert(child.id, child.block_type);
        if !self.ensure_editor(parent) {
            return;
        }
        self.pending_transfers.push(PendingTransfer {
            child,
            source: None,
            destination: Some(parent),
            parent_after: Some(BlockParent::Uuid(parent)),
            stage: TransferStage::AddDestination,
        });
    }

    fn queue_move(&mut self, dragged: SidebarDragPayload, destination: Uuid) {
        if self.pending_transfers.iter().any(|pending| {
            pending.child.id == dragged.reference.id && pending.destination == Some(destination)
        }) {
            return;
        }
        self.block_types
            .insert(dragged.reference.id, dragged.reference.block_type);
        let source_ready = match dragged.source {
            SidebarDragSource::Root | SidebarDragSource::Orphaned => true,
            SidebarDragSource::Block(source) => self.ensure_editor(source),
        };
        if !source_ready || !self.ensure_editor(destination) {
            return;
        }
        self.pending_transfers.push(PendingTransfer {
            child: dragged.reference,
            source: Some(dragged.source),
            destination: Some(destination),
            parent_after: (!dragged.is_reference).then_some(BlockParent::Uuid(destination)),
            stage: TransferStage::DeleteSource,
        });
    }

    fn queue_delete(
        &mut self,
        reference: BlockReference,
        source: SidebarDragSource,
        is_reference: bool,
    ) {
        if self.pending_transfers.iter().any(|pending| {
            pending.child.id == reference.id
                && pending.source == Some(source)
                && pending.destination.is_none()
        }) {
            return;
        }
        self.block_types.insert(reference.id, reference.block_type);
        let ready = match source {
            SidebarDragSource::Root | SidebarDragSource::Orphaned => true,
            SidebarDragSource::Block(source) => self.ensure_editor(source),
        };
        if !ready {
            return;
        }
        self.pending_transfers.push(PendingTransfer {
            child: reference,
            source: Some(source),
            destination: None,
            parent_after: (!is_reference && source != SidebarDragSource::Root)
                .then_some(BlockParent::Orphaned),
            stage: TransferStage::DeleteSource,
        });
    }

    fn process_pending_transfers(&mut self) {
        let pending = std::mem::take(&mut self.pending_transfers);
        for mut transfer in pending {
            if matches!(transfer.stage, TransferStage::DeleteSource) {
                let ready = match transfer.source {
                    None | Some(SidebarDragSource::Orphaned) => Some(true),
                    Some(SidebarDragSource::Root) => {
                        self.client
                            .set_block_parent(transfer.child.id, BlockParent::Orphaned);
                        Some(true)
                    }
                    Some(SidebarDragSource::Block(source)) => {
                        self.editors.get(&source).and_then(|editor| {
                            editor.delete_child(BlockEntry {
                                id: transfer.child.id,
                            })
                        })
                    }
                };
                if ready != Some(true) {
                    self.pending_transfers.push(transfer);
                    continue;
                }
                transfer.stage = TransferStage::AddDestination;
            }

            let ready = transfer.destination.map_or(Some(true), |destination| {
                self.editors.get(&destination).and_then(|editor| {
                    editor.add_child(BlockEntry {
                        id: transfer.child.id,
                    })
                })
            });
            if ready != Some(true) {
                self.pending_transfers.push(transfer);
                continue;
            }

            if let Some(parent) = transfer.parent_after {
                if let BlockParent::Uuid(destination) = parent {
                    if let Some(child) = self.editors.get(&transfer.child.id) {
                        child.note_backref(destination);
                    }
                }
                self.client.set_block_parent(transfer.child.id, parent);
            }
        }
    }

    fn open_tab(&mut self, id: Uuid, block_type: Uuid) {
        self.block_types.insert(id, block_type);
        if !self.editors.contains_key(&id) {
            self.editors
                .insert(id, self.registry.open(&self.client, id, block_type));
        }
        self.parents
            .entry(id)
            .or_insert_with(|| self.client.watch_parents(id));
        self.references.entry(id).or_insert_with(|| {
            self.client
                .watch_references(BlockReferenceList::References(id))
        });
        self.backrefs.entry(id).or_insert_with(|| {
            self.client
                .watch_references(BlockReferenceList::Backrefs(id))
        });
        let tab = DockTab::Block(id);
        if self.dock_state.find_tab(&tab).is_none() {
            let existing_block = self
                .dock_state
                .iter_all_tabs()
                .find_map(|(path, tab)| matches!(tab, DockTab::Block(_)).then_some(path));
            if let Some(empty_path) = self.dock_state.find_tab(&DockTab::Empty) {
                let leaf = self
                    .dock_state
                    .leaf_mut(empty_path.node_path())
                    .expect("blank workspace must be a dock leaf");
                leaf.tabs_mut()[empty_path.tab.0] = tab;
            } else if let Some(path) = existing_block {
                self.dock_state
                    .set_focused_node_and_surface(path.node_path());
                self.dock_state.push_to_focused_leaf(tab);
            } else if let Some(files_path) = self.dock_state.find_tab(&DockTab::Files) {
                self.dock_state[files_path.surface].split_right(files_path.node, 0.22, vec![tab]);
            } else {
                self.dock_state.push_to_focused_leaf(tab);
            }
        }
        if let Some(path) = self.dock_state.find_tab(&tab) {
            let _ = self.dock_state.set_active_tab(path);
            self.dock_state
                .set_focused_node_and_surface(path.node_path());
        }
        if self.active_tab != Some(id) {
            self.sidebar_reveal = None;
        }
        self.active_tab = Some(id);
    }

    fn close_tab_resources(&mut self, id: Uuid) {
        self.parents.remove(&id);
        self.references.remove(&id);
        self.backrefs.remove(&id);
        self.dynamic_artifact_regenerations.remove(&id);
        self.dynamic_artifact_errors.remove(&id);
        if let Some(editor) = self.editors.get_mut(&id) {
            editor.tab_closed();
        }
        if self.active_tab == Some(id) {
            self.active_tab = None;
            self.sidebar_reveal = None;
        }
    }

    fn sidebar_active_location(&self) -> Option<SidebarActiveLocation> {
        let id = self.active_tab?;
        let editor = self.editors.get(&id)?;
        let parents = self.parents.get(&id)?;
        if !parents.is_loaded() {
            return None;
        }
        let parents = parents.read();
        let root = if let Some(parent) = parents.first() {
            parent.parent
        } else if let Some(relationships) = editor.relationships() {
            relationships.parent
        } else {
            if !self.roots.is_loaded() {
                return None;
            }
            if self.roots.read().iter().any(|root| root.id == id) {
                BlockParent::Root
            } else {
                BlockParent::Orphaned
            }
        };
        if matches!(root, BlockParent::Uuid(_)) {
            return None;
        }
        let mut path = parents.iter().map(|parent| parent.id).collect::<Vec<_>>();
        path.push(id);
        Some(SidebarActiveLocation {
            id,
            name: self
                .client
                .cached_block(id)
                .map_or_else(|| editor.name(), |block| block.name),
            root,
            path,
        })
    }

    fn reveal_sidebar_location(&mut self, location: &SidebarActiveLocation) {
        if location.root == BlockParent::Orphaned && !self.orphaned_expanded {
            self.orphaned_expanded = true;
            self.orphaned = Some(self.client.watch_references(BlockReferenceList::Orphans));
        }
        for parent in location
            .path
            .iter()
            .take(location.path.len().saturating_sub(1))
        {
            self.expanded.entry(*parent).or_insert_with(|| {
                self.client
                    .watch_references(BlockReferenceList::References(*parent))
            });
        }
        self.sidebar_reveal = Some(location.id);
    }

    fn reference_label(&self, reference: &BlockReference) -> String {
        reference.name.clone()
    }

    fn can_delete_from(&self, source: SidebarDragSource) -> bool {
        match source {
            SidebarDragSource::Root | SidebarDragSource::Orphaned => true,
            SidebarDragSource::Block(id) => self
                .block_types
                .get(&id)
                .is_some_and(|block_type| self.registry.can_delete_child(*block_type)),
        }
    }

    fn collapse_reference(&mut self, id: Uuid) {
        self.expanded.remove(&id);
    }

    fn show_reference(
        &mut self,
        ui: &mut egui::Ui,
        reference: BlockReference,
        depth: usize,
        containing_id: Option<Uuid>,
        path: &mut HashSet<Uuid>,
        active: Option<&SidebarActiveLocation>,
        sidebar: &mut SidebarRenderState,
    ) {
        self.block_types.insert(reference.id, reference.block_type);
        let is_reference =
            containing_id.is_some_and(|id| reference.parent != BlockParent::Uuid(id));
        let source = containing_id.map_or_else(
            || match reference.parent {
                BlockParent::Orphaned => SidebarDragSource::Orphaned,
                BlockParent::Root | BlockParent::Uuid(_) => SidebarDragSource::Root,
            },
            SidebarDragSource::Block,
        );
        let can_add_child = self.registry.can_add_child(reference.block_type);
        let can_delete_child =
            source != SidebarDragSource::Orphaned && self.can_delete_from(source);
        let can_expand = !is_reference && reference.references > 0;
        let was_expanded = self.expanded.contains_key(&reference.id);
        let is_active = !is_reference && active.is_some_and(|active| active.id == reference.id);
        let is_closed_active_ancestor = !is_reference
            && !was_expanded
            && active.is_some_and(|active| {
                active.id != reference.id && active.path.contains(&reference.id)
            });
        let mut toggle = false;
        let mut open = false;
        let mut delete = false;
        let mut picker_action = None;
        let mut row_response = None;
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 14.0);
            let expand_response = ui
                .add_enabled(
                    can_expand,
                    egui::Button::selectable(
                        is_closed_active_ancestor,
                        if can_expand {
                            if was_expanded {
                                "▾"
                            } else {
                                "▸"
                            }
                        } else {
                            if is_reference {
                                "→"
                            } else {
                                "•"
                            }
                        },
                    )
                    .small(),
                )
                .on_hover_text(match (can_expand, is_reference) {
                    (false, true) => format!(
                        "{}\nThis block is referenced here, but is not a direct child. It cannot be expanded here.",
                        reference.id
                    ),
                    (false, false) => {
                        format!(
                            "{}\nThis block cannot be expanded because it has no children.",
                            reference.id
                        )
                    }
                    (true, _) => reference.id.to_string(),
                });
            if expand_response.clicked() {
                toggle = true;
            }
            if is_closed_active_ancestor {
                sidebar.highlight = Some(SidebarHighlight {
                    rect: expand_response.rect,
                    collapsed: true,
                });
            }
            let trailing_width = if can_add_child {
                ui.spacing().interact_size.x + ui.spacing().item_spacing.x
            } else {
                0.0
            };
            let label_width = (ui.available_width() - trailing_width).max(0.0);
            let response = ui.add_sized(
                [label_width, ui.spacing().interact_size.y],
                egui::Button::selectable(
                    is_active,
                    self.reference_label(&reference),
                )
                .right_text(())
                .truncate()
                .sense(egui::Sense::click_and_drag()),
            );
            response.dnd_set_drag_payload(SidebarDragPayload {
                reference: reference.clone(),
                source,
                is_reference,
            });
            if response.clicked() {
                open = true;
            }
            if is_active {
                if self.sidebar_reveal == Some(reference.id) {
                    response.scroll_to_me(Some(egui::Align::Center));
                    self.sidebar_reveal = None;
                    sidebar.scroll_requested = true;
                }
                sidebar.highlight = Some(SidebarHighlight {
                    rect: response.rect,
                    collapsed: false,
                });
            }
            response.context_menu(|ui| {
                match block_context_menu(ui, &self.registry, can_add_child, can_delete_child) {
                    Some(BlockContextMenuAction::Picker(action)) => picker_action = Some(action),
                    Some(BlockContextMenuAction::Rename) => {
                        self.rename = Some(RenameState {
                            id: reference.id,
                            name: reference.name.clone(),
                        });
                    }
                    Some(BlockContextMenuAction::Delete) => delete = true,
                    None => {}
                }
            });
            row_response = Some(response);
            if can_add_child {
                ui.menu_button("+", |ui| {
                    picker_action = BlockPicker::show_menu(ui, &self.registry);
                })
                .response
                .on_hover_text("Add a child");
            }
        });

        if let Some(action) = picker_action {
            let mut excluded = path.clone();
            excluded.insert(reference.id);
            self.handle_picker_menu_action(
                action,
                BlockPickerTarget::Block(reference.id),
                excluded,
            );
        }
        if delete {
            self.queue_delete(reference.clone(), source, is_reference);
        }
        if can_add_child {
            if let Some(response) = row_response {
                if let Some(dragged) = response.dnd_hover_payload::<SidebarDragPayload>() {
                    let valid = dragged.reference.id != reference.id
                        && dragged.source != SidebarDragSource::Block(reference.id)
                        && !path.contains(&dragged.reference.id)
                        && self.can_delete_from(dragged.source);
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
                if let Some(dragged) = response.dnd_release_payload::<SidebarDragPayload>() {
                    if dragged.reference.id != reference.id
                        && dragged.source != SidebarDragSource::Block(reference.id)
                        && !path.contains(&dragged.reference.id)
                        && self.can_delete_from(dragged.source)
                    {
                        self.queue_move(dragged.as_ref().clone(), reference.id);
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

        let is_expanded = can_expand && self.expanded.contains_key(&reference.id);
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
            self.show_reference(
                ui,
                child,
                depth + 1,
                Some(reference.id),
                path,
                active,
                sidebar,
            );
        }
        path.remove(&reference.id);
    }

    fn show_sidebar(&mut self, ui: &mut egui::Ui) {
        if self.sidebar_reveal.is_some() && self.sidebar_reveal != self.active_tab {
            self.sidebar_reveal = None;
        }
        let active = self.sidebar_active_location();
        let mut picker_action = None;
        ui.horizontal(|ui| {
            ui.heading("Blocks");
            ui.add_space(ui.available_width() - 28.0);
            ui.menu_button("+", |ui| {
                picker_action = BlockPicker::show_menu(ui, &self.registry);
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

        let mut sidebar = SidebarRenderState::default();
        let scroll = egui::ScrollArea::vertical().show(ui, |ui| {
            let roots = self.roots.read();
            if roots.is_empty() && self.roots.is_loaded() {
                ui.weak("No root blocks");
            }
            for root in roots {
                self.show_reference(
                    ui,
                    root,
                    0,
                    None,
                    &mut HashSet::new(),
                    active.as_ref(),
                    &mut sidebar,
                );
            }
            ui.separator();
            self.show_recently_deleted(ui, active.as_ref(), &mut sidebar);
        });
        if sidebar.scroll_requested {
            return;
        }
        let Some(active) = active else {
            return;
        };
        let Some(highlight) = sidebar.highlight else {
            return;
        };
        let viewport = scroll.inner_rect;
        let at_top = highlight.rect.bottom() < viewport.top();
        let at_bottom = highlight.rect.top() > viewport.bottom();
        let (arrow, y) = if at_top {
            ("↑", viewport.top())
        } else if at_bottom {
            ("↓", viewport.bottom() - ui.spacing().interact_size.y)
        } else if highlight.collapsed {
            ("↑", viewport.bottom() - ui.spacing().interact_size.y)
        } else {
            return;
        };
        let rect = egui::Rect::from_min_size(
            egui::pos2(viewport.left(), y),
            egui::vec2(viewport.width(), ui.spacing().interact_size.y),
        );
        if ui
            .put(
                rect,
                egui::Button::new(format!("{arrow} ({})", active.name)),
            )
            .clicked()
        {
            self.reveal_sidebar_location(&active);
        }
    }

    fn show_recently_deleted(
        &mut self,
        ui: &mut egui::Ui,
        active: Option<&SidebarActiveLocation>,
        sidebar: &mut SidebarRenderState,
    ) {
        let mut toggle = false;
        ui.horizontal(|ui| {
            let is_closed_active_ancestor = !self.orphaned_expanded
                && active.is_some_and(|active| active.root == BlockParent::Orphaned);
            let expand_response = ui.add(
                egui::Button::selectable(
                    is_closed_active_ancestor,
                    if self.orphaned_expanded {
                        "\u{25bc}"
                    } else {
                        "\u{25b6}"
                    },
                )
                .small(),
            );
            if expand_response.clicked() {
                toggle = true;
            }
            if is_closed_active_ancestor {
                sidebar.highlight = Some(SidebarHighlight {
                    rect: expand_response.rect,
                    collapsed: true,
                });
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
            self.show_reference(ui, block, 1, None, &mut HashSet::new(), active, sidebar);
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
                    if ui.button("Client").clicked() {
                        self.client_debug_open = true;
                        ui.close();
                    }
                    if ui.button("Network").clicked() {
                        self.network_debug_open = true;
                        ui.close();
                    }
                    ui.separator();
                    ui.strong("Accounts");
                    ui.small(format!("Signed in as {}", self.account.name));
                    ui.small(self.account.id.to_string());
                    for account in ACCOUNTS {
                        if ui
                            .selectable_label(account == self.account, account.name)
                            .on_hover_text(account.id.to_string())
                            .clicked()
                        {
                            self.request_account_switch(account);
                            ui.close();
                        }
                    }
                });
            });
        });
    }

    fn show_content(
        &mut self,
        ui: &mut egui::Ui,
        frame: &eframe::Frame,
        active: Uuid,
    ) -> Option<EditorAction> {
        let Some(mut editor) = self.editors.remove(&active) else {
            return None;
        };
        self.show_dynamic_artifact_bar(ui, active, editor.block_type());
        if let Some(history) = editor
            .history()
            .filter(|history| history.supports_history())
        {
            let undo_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
            let redo_shortcut = egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Z,
            );
            let redo_y_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Y);
            let redo_requested = ui
                .ctx()
                .input_mut(|input| input.consume_shortcut(&redo_shortcut))
                || ui
                    .ctx()
                    .input_mut(|input| input.consume_shortcut(&redo_y_shortcut));
            let undo_requested = ui
                .ctx()
                .input_mut(|input| input.consume_shortcut(&undo_shortcut));
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(history.can_undo(), egui::Button::new("Undo"))
                    .on_hover_text("Undo (Ctrl/Cmd+Z)")
                    .clicked()
                    || undo_requested
                {
                    history.undo();
                }
                if ui
                    .add_enabled(history.can_redo(), egui::Button::new("Redo"))
                    .on_hover_text("Redo (Ctrl+Y or Ctrl/Cmd+Shift+Z)")
                    .clicked()
                    || redo_requested
                {
                    history.redo();
                }
            });
            ui.separator();
        }
        let action = {
            let mut editors =
                EditorAccess::new(active, &self.client, &self.registry, &mut self.editors);
            editor.ui(ui, &mut editors, frame)
        };
        self.editors.insert(active, editor);
        action
    }

    fn show_dynamic_artifact_bar(&mut self, ui: &mut egui::Ui, id: Uuid, block_type: Uuid) {
        let Some(descriptor) = self.client.dynamic_artifact(id) else {
            return;
        };

        let completed = self
            .dynamic_artifact_regenerations
            .get_mut(&id)
            .and_then(|regeneration| regeneration.poll());
        if let Some(result) = completed {
            self.dynamic_artifact_regenerations.remove(&id);
            match result {
                Ok(()) => {
                    self.dynamic_artifact_errors.remove(&id);
                }
                Err(error) => {
                    self.dynamic_artifact_errors.insert(id, error);
                }
            }
        }

        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(egui::Margin::symmetric(8, 5))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Dynamic artifact");
                    ui.weak("Generated from another block.");
                    let running = self.dynamic_artifact_regenerations.contains_key(&id);
                    if running {
                        ui.spinner();
                    }
                    if ui
                        .add_enabled(!running, egui::Button::new("Regenerate"))
                        .clicked()
                    {
                        self.dynamic_artifact_errors.remove(&id);
                        match self.registry.regenerate_dynamic_artifact(
                            descriptor.source_type,
                            &self.client,
                            id,
                            block_type,
                            &descriptor.data,
                        ) {
                            Ok(regeneration) => {
                                self.dynamic_artifact_regenerations.insert(id, regeneration);
                            }
                            Err(error) => {
                                self.dynamic_artifact_errors.insert(id, error);
                            }
                        }
                    }
                });
                if let Some(error) = self.dynamic_artifact_errors.get(&id) {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });
        ui.separator();
        if self.dynamic_artifact_regenerations.contains_key(&id) {
            ui.ctx().request_repaint();
        }
    }

    fn handle_editor_action(&mut self, active: Uuid, action: EditorAction) {
        match action {
            EditorAction::OpenBlock { id, block_type } => self.open_tab(id, block_type),
            EditorAction::CreateBlock { block_type, parent } => {
                if let Some(id) = self.create_block_editor(block_type) {
                    let name = self
                        .registry
                        .display_name(block_type)
                        .unwrap_or_default()
                        .to_owned();
                    let referenced = self.editors.get_mut(&active).is_some_and(|editor| {
                        editor.block_created(id, block_type, self.account.id, name)
                    });
                    match (parent, referenced) {
                        (Some(parent), true) => {
                            if let Some(created) = self.editors.get(&id) {
                                created.note_backref(parent);
                                created.set_parent(BlockParent::Uuid(parent));
                            }
                        }
                        (None, _) => {
                            if let Some(created) = self.editors.get(&id) {
                                created.set_parent(BlockParent::Root);
                            }
                        }
                        (Some(_), false) => {}
                    }
                }
            }
            EditorAction::SetParent { id, parent } => {
                if let Some(child) = self.editors.get(&id) {
                    child.note_backref(parent);
                    child.set_parent(BlockParent::Uuid(parent));
                }
            }
        }
    }

    fn show_dock(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let mut dock_state = std::mem::replace(&mut self.dock_state, default_dock_state());
        for (_, tab) in dock_state.iter_all_tabs() {
            if let DockTab::Block(id) = tab {
                if let Some(editor) = self.editors.get_mut(id) {
                    editor.update_open_tab(frame);
                    editor.set_tab_active(false);
                }
            }
        }
        let mut viewer = BlockTabViewer {
            app: self,
            frame,
            actions: Vec::new(),
            tabs_to_close: Vec::new(),
        };
        DockArea::new(&mut dock_state).show_inside(ui, &mut viewer);
        let tabs_to_close = std::mem::take(&mut viewer.tabs_to_close);
        for id in tabs_to_close {
            let Some(path) = dock_state.find_tab(&DockTab::Block(id)) else {
                continue;
            };
            let editor_count = dock_state
                .iter_all_tabs()
                .filter(|(_, tab)| matches!(tab, DockTab::Block(_)))
                .count();
            if editor_count == 1 {
                let leaf = dock_state
                    .leaf_mut(path.node_path())
                    .expect("editor tab must be in a dock leaf");
                leaf.tabs_mut()[path.tab.0] = DockTab::Empty;
            } else {
                dock_state.remove_tab(path);
            }
            viewer.app.close_tab_resources(id);
        }
        ensure_empty_workspace(&mut dock_state);
        let actions = std::mem::take(&mut viewer.actions);
        let pending_tabs = viewer
            .app
            .dock_state
            .iter_all_tabs()
            .filter_map(|(_, tab)| match tab {
                DockTab::Files | DockTab::Empty => None,
                DockTab::Block(id) => Some(*id),
            })
            .collect::<Vec<_>>();
        let previous_active = viewer.app.active_tab;
        let active_tab = dock_state
            .find_active_focused()
            .and_then(|(_, tab)| match tab {
                DockTab::Files | DockTab::Empty => None,
                DockTab::Block(id) => Some(*id),
            })
            .or_else(|| {
                previous_active.filter(|id| dock_state.find_tab(&DockTab::Block(*id)).is_some())
            })
            .or_else(|| {
                dock_state.iter_all_tabs().find_map(|(_, tab)| match tab {
                    DockTab::Files | DockTab::Empty => None,
                    DockTab::Block(id) => Some(*id),
                })
            });
        viewer.app.dock_state = dock_state;
        if previous_active != active_tab {
            viewer.app.sidebar_reveal = None;
        }
        viewer.app.active_tab = active_tab;
        for id in pending_tabs {
            if let Some(block_type) = viewer.app.block_types.get(&id).copied() {
                viewer.app.open_tab(id, block_type);
            }
        }
        for (active, action) in actions {
            viewer.app.handle_editor_action(active, action);
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
        let parents = self.parents.get(&active).map(ReferenceList::read);
        let references = self
            .references
            .get(&active)
            .map(|list| (list.is_loaded(), list.read()));
        let backrefs = self
            .backrefs
            .get(&active)
            .map(|list| (list.is_loaded(), list.read()));
        let current_name = editor.name();
        let mut navigate = None;
        let mut context_action = None;

        ui.horizontal_wrapped(|ui| {
            ui.small(format!("Type: {type_name}"));
            ui.separator();
            let Some(relationships) = &relationships else {
                ui.small("Relationships loading…");
                return;
            };

            let Some(parents) = &parents else {
                ui.small("Parents loading…");
                return;
            };
            let root = parents
                .first()
                .map_or(relationships.parent, |parent| parent.parent);
            ui.small(match root {
                BlockParent::Orphaned => "Recently Deleted",
                BlockParent::Root => "Root",
                BlockParent::Uuid(_) => "Unknown",
            });
            for parent in parents {
                self.record_reference_types(parent);
                let source = sidebar_source(parent.parent);
                ui.small("›");
                let response = ui
                    .small_button(&parent.name)
                    .on_hover_text(parent.id.to_string());
                if response.clicked() {
                    navigate = Some((parent.id, parent.block_type));
                }
                let can_add = self.registry.can_add_child(parent.block_type);
                let can_delete =
                    source != SidebarDragSource::Orphaned && self.can_delete_from(source);
                response.context_menu(|ui| {
                    if let Some(action) =
                        block_context_menu(ui, &self.registry, can_add, can_delete)
                    {
                        context_action = Some((parent.clone(), source, false, action));
                    }
                });
            }
            ui.small("›");
            ui.small(current_name);
            ui.separator();
            ui.menu_button(
                format!(
                    "Backrefs: {}",
                    backrefs
                        .as_ref()
                        .map_or(relationships.backrefs.len(), |(loaded, backrefs)| {
                            if *loaded {
                                backrefs.len()
                            } else {
                                relationships.backrefs.len()
                            }
                        })
                ),
                |ui| {
                    let Some((loaded, backrefs)) = &backrefs else {
                        ui.weak("Loading…");
                        return;
                    };
                    self.status_reference_list(
                        ui,
                        backrefs,
                        *loaded,
                        "No backrefs",
                        None,
                        &mut navigate,
                        &mut context_action,
                    );
                },
            );
            ui.separator();
            ui.menu_button(
                format!(
                    "References: {}",
                    references.as_ref().map_or(
                        relationships.references.len(),
                        |(loaded, references)| {
                            if *loaded {
                                references.len()
                            } else {
                                relationships.references.len()
                            }
                        }
                    )
                ),
                |ui| {
                    let Some((loaded, references)) = &references else {
                        ui.weak("Loading…");
                        return;
                    };
                    self.status_reference_list(
                        ui,
                        references,
                        *loaded,
                        "No references",
                        Some(active),
                        &mut navigate,
                        &mut context_action,
                    );
                },
            );
        });

        if let Some((reference, source, is_reference, action)) = context_action {
            match action {
                BlockContextMenuAction::Picker(action) => {
                    self.handle_picker_menu_action(
                        action,
                        BlockPickerTarget::Block(reference.id),
                        [reference.id],
                    );
                }
                BlockContextMenuAction::Rename => {
                    self.rename = Some(RenameState {
                        id: reference.id,
                        name: reference.name,
                    });
                }
                BlockContextMenuAction::Delete => {
                    self.queue_delete(reference, source, is_reference);
                }
            }
        }
        if let Some((id, block_type)) = navigate {
            self.open_tab(id, block_type);
        }
    }

    fn record_reference_types(&mut self, reference: &BlockReference) {
        self.block_types.insert(reference.id, reference.block_type);
        if let BlockParent::Uuid(parent) = reference.parent {
            if let Some(parent) = self.client.cached_block(parent) {
                self.block_types.insert(parent.id, parent.block_type);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn status_reference_list(
        &mut self,
        ui: &mut egui::Ui,
        references: &[BlockReference],
        loaded: bool,
        empty: &str,
        containing_id: Option<Uuid>,
        navigate: &mut Option<(Uuid, Uuid)>,
        context_action: &mut Option<(
            BlockReference,
            SidebarDragSource,
            bool,
            BlockContextMenuAction,
        )>,
    ) {
        if references.is_empty() {
            ui.weak(if loaded { empty } else { "Loading…" });
        }
        for reference in references {
            self.record_reference_types(reference);
            let source = containing_id.map_or_else(
                || sidebar_source(reference.parent),
                SidebarDragSource::Block,
            );
            let is_reference =
                containing_id.is_some_and(|id| reference.parent != BlockParent::Uuid(id));
            let response = ui
                .button(&reference.name)
                .on_hover_text(reference.id.to_string());
            if response.clicked() {
                *navigate = Some((reference.id, reference.block_type));
                ui.close();
            }
            let can_add = self.registry.can_add_child(reference.block_type);
            let can_delete = source != SidebarDragSource::Orphaned && self.can_delete_from(source);
            response.context_menu(|ui| {
                if let Some(action) = block_context_menu(ui, &self.registry, can_add, can_delete) {
                    *context_action = Some((reference.clone(), source, is_reference, action));
                }
            });
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

struct BlockTabViewer<'a> {
    app: &'a mut BlockApp,
    frame: &'a eframe::Frame,
    actions: Vec<(Uuid, EditorAction)>,
    tabs_to_close: Vec<Uuid>,
}

impl TabViewer for BlockTabViewer<'_> {
    type Tab = DockTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            DockTab::Files => "Files".into(),
            DockTab::Empty => "Workspace".into(),
            DockTab::Block(id) => self
                .app
                .editors
                .get(id)
                .map_or_else(|| id.to_string(), |editor| editor.name())
                .into(),
        }
    }

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            DockTab::Files => {
                let content_rect = ui.available_rect_before_wrap();
                let mut content_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("blocks-sidebar-content")
                        .max_rect(content_rect),
                );
                content_ui.set_clip_rect(content_rect.intersect(ui.clip_rect()));
                content_ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                self.app.show_sidebar(&mut content_ui);
                ui.advance_cursor_after_rect(content_rect);
            }
            DockTab::Empty => {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("No file open");
                        ui.weak("Open or create a file from Files to get started.");
                    });
                });
            }
            DockTab::Block(id) => {
                if let Some(editor) = self.app.editors.get_mut(id) {
                    editor.set_tab_active(true);
                }
                if let Some(action) = self.app.show_content(ui, self.frame, *id) {
                    self.actions.push((*id, action));
                }
            }
        }
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> OnCloseResponse {
        match tab {
            DockTab::Files | DockTab::Empty => OnCloseResponse::Ignore,
            DockTab::Block(id) => {
                self.tabs_to_close.push(*id);
                OnCloseResponse::Ignore
            }
        }
    }

    fn is_closeable(&self, tab: &Self::Tab) -> bool {
        matches!(tab, DockTab::Block(_))
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [false, false]
    }
}

enum BlockContextMenuAction {
    Picker(BlockPickerMenuAction),
    Rename,
    Delete,
}

fn block_context_menu(
    ui: &mut egui::Ui,
    registry: &EditorRegistry,
    can_add: bool,
    can_delete: bool,
) -> Option<BlockContextMenuAction> {
    let mut action = None;
    ui.add_enabled_ui(can_add, |ui| {
        ui.menu_button("Add", |ui| {
            if let Some(picker_action) = BlockPicker::show_menu(ui, registry) {
                action = Some(BlockContextMenuAction::Picker(picker_action));
            }
        });
    });
    if ui.button("Rename").clicked() {
        action = Some(BlockContextMenuAction::Rename);
        ui.close();
    }
    let delete_text = egui::RichText::new("Delete");
    let delete_text = if can_delete {
        delete_text.color(ui.visuals().error_fg_color)
    } else {
        delete_text
    };
    if ui
        .add_enabled(can_delete, egui::Button::new(delete_text))
        .clicked()
    {
        action = Some(BlockContextMenuAction::Delete);
        ui.close();
    }
    action
}

fn sidebar_source(parent: BlockParent) -> SidebarDragSource {
    match parent {
        BlockParent::Root => SidebarDragSource::Root,
        BlockParent::Orphaned => SidebarDragSource::Orphaned,
        BlockParent::Uuid(parent) => SidebarDragSource::Block(parent),
    }
}

impl eframe::App for BlockApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if let Some(account) = self.scheduled_account_switch.take() {
            self.switch_account(ui.ctx(), account);
        }
        self.intercept_close(ui.ctx());
        self.process_pending_transfers();
        self.show_block_picker(ui.ctx());
        self.show_rename(ui);
        self.show_client_debug(ui.ctx());
        self.show_network_debug(ui.ctx());

        ui.vertical(|ui| {
            egui::Panel::bottom("block-statusbar")
                .resizable(false)
                .show_inside(ui, |ui| self.show_statusbar(ui));
            self.show_dock(ui, frame);
        });
        self.show_discard_confirmation(ui.ctx());
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
