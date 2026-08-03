mod app_state;
mod block_picker;
mod debug;
mod editors;
mod performance;

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    io,
    net::TcpListener as StdTcpListener,
    path::PathBuf,
    thread,
    time::Duration,
};

use app_state::{AppStateStore, SavedAccount, ServerLocation};
use block::{BlockParent, BlockReference, BlockReferenceList, MAX_NAME_BYTES};
use block_client::{
    blocks::workspace_index::BlockEntry, BlockClient, ManagementClient, ReferenceList,
};
use block_picker::{BlockPicker, BlockPickerResult};
use editors::{
    direct_editor_tab_ui, BlockEditor, DynamicArtifactRegeneration, EditorAccess, EditorAction,
    EditorRegistry, SidebarDragPayload, SidebarDragSource,
};
use eframe::egui;
use egui_dock::{widgets::tab_viewer::OnCloseResponse, DockArea, DockState, TabViewer};
use egui_material_icons::icons::{
    ICON_ADD, ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_ARROW_UPWARD, ICON_CHECK,
    ICON_CHEVRON_RIGHT, ICON_CIRCLE, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT,
    ICON_REDO, ICON_REFRESH, ICON_UNDO,
};
use tokio::net::TcpListener;
use uuid::Uuid;

const APP_ID: &str = "Block";
const COMPACT_FILES_WIDTH: f32 = 700.0;
fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID)
            .with_inner_size([1100.0, 720.0]),
        ..Default::default()
    }
}

fn run_native(options: eframe::NativeOptions, storage_root: Option<PathBuf>) -> eframe::Result {
    eframe::run_native(
        APP_ID,
        options,
        Box::new(move |creation_context| {
            egui_material_icons::initialize(&creation_context.egui_ctx);
            BlockApp::new(storage_root)
                .map(|app| Box::new(app) as Box<dyn eframe::App>)
                .map_err(Into::into)
        }),
    )
}

#[cfg(not(target_os = "android"))]
pub fn run() -> eframe::Result {
    run_native(native_options(), None)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    let storage_root = app.internal_data_path();
    let mut options = native_options();
    options.android_app = Some(app);
    let exit_code = match run_native(options, storage_root) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Block stopped: {error}");
            1
        }
    };

    // android-activity may start a later Activity instance on a new android_main thread in the
    // same process. Eframe stores its event loop in thread-local state while winit permits only
    // one event loop per process, so that later instance cannot reuse the original loop. End this
    // native-only process once the loop exits so Android starts the next Activity in a clean one.
    std::process::exit(exit_code);
}

struct BlockApp {
    app_state: AppStateStore,
    local_server_url: String,
    accounts: Vec<Account>,
    signed_in: bool,
    account_form: AccountForm,
    pending_account_request: Option<PendingAccountRequest>,
    account_error: Option<String>,
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
    files_compact: bool,
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

#[derive(Clone, PartialEq, Eq, Hash)]
enum DockTab {
    Files,
    Empty,
    Block(BlockTab),
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct BlockTab {
    id: Uuid,
    history: Vec<BlockTabHistoryItem>,
    history_index: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BlockTabHistoryItem {
    id: Uuid,
    block_type: Uuid,
}

impl BlockTab {
    fn new(id: Uuid, block_type: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            history: vec![BlockTabHistoryItem { id, block_type }],
            history_index: 0,
        }
    }

    fn current(&self) -> BlockTabHistoryItem {
        self.history[self.history_index]
    }

    fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }

    fn navigate(&mut self, item: BlockTabHistoryItem) {
        if self.current().id == item.id {
            return;
        }
        self.history.truncate(self.history_index + 1);
        self.history.push(item);
        self.history_index += 1;
    }

    fn go_back(&mut self) {
        if self.can_go_back() {
            self.history_index -= 1;
        }
    }

    fn go_forward(&mut self) {
        if self.can_go_forward() {
            self.history_index += 1;
        }
    }
}

#[derive(Clone, Copy)]
enum TabNavigation {
    Back,
    Forward,
    Open(BlockTabHistoryItem),
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

fn set_files_compact(dock_state: &mut DockState<DockTab>, compact: bool) {
    let active = dock_state
        .find_active_focused()
        .map(|(_, tab)| tab.clone())
        .unwrap_or(DockTab::Files);
    let Some(files_path) = dock_state.find_tab(&DockTab::Files) else {
        return;
    };
    let target = dock_state
        .iter_all_tabs()
        .find_map(|(path, tab)| (*tab != DockTab::Files).then_some(path.node_path()));
    let Some(target) = target else {
        return;
    };

    dock_state.set_focused_node_and_surface(target);
    dock_state.remove_tab(files_path);
    if compact {
        dock_state.push_to_focused_leaf(DockTab::Files);
    } else if let Some(target) = dock_state.find_tab(&active).or_else(|| {
        dock_state
            .iter_all_tabs()
            .find_map(|(path, tab)| (*tab != DockTab::Files).then_some(path))
    }) {
        dock_state[target.surface].split_left(target.node, 0.78, vec![DockTab::Files]);
    }

    if let Some(active_path) = dock_state.find_tab(&active) {
        let _ = dock_state.set_active_tab(active_path);
        dock_state.set_focused_node_and_surface(active_path.node_path());
    }
}

type Account = SavedAccount;

#[derive(Default)]
struct AccountForm {
    remote: bool,
    remote_url: String,
    register: bool,
    email: String,
    display_name: String,
}

struct PendingAccountRequest {
    receiver: std::sync::mpsc::Receiver<Result<block::Account, String>>,
    server: ServerLocation,
    url: String,
}

#[derive(Clone)]
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
    fn new(storage_root: Option<PathBuf>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let data_dir = storage_root
            .or_else(|| eframe::storage_dir(APP_ID))
            .ok_or_else(|| io::Error::other("application-data directory is unavailable"))?;
        std::fs::create_dir_all(&data_dir)?;
        let url = start_embedded_server(data_dir.join("server"))?;
        let app_state = AppStateStore::open(data_dir.join("app.sqlite3"))?;
        let accounts = app_state.accounts()?;
        let active = app_state.active_account()?;
        let account = active
            .and_then(|(server, id)| {
                accounts
                    .iter()
                    .find(|account| account.server.key() == server && account.id == id)
            })
            .cloned();
        let signed_in = account.is_some();
        let account = account.unwrap_or_else(|| Account {
            server: ServerLocation::Local,
            id: Uuid::nil(),
            email: String::new(),
            name: String::new(),
            last_workspace_id: None,
        });
        let server_url = match &account.server {
            ServerLocation::Local => url.clone(),
            ServerLocation::Remote(remote) => remote.clone(),
        };
        let client = BlockClient::new(account.id);
        if signed_in {
            client.connect(server_url.clone());
        }
        let roots = client.watch_references(BlockReferenceList::Roots);
        Ok(Self {
            app_state,
            local_server_url: url,
            accounts,
            signed_in,
            account_form: AccountForm::default(),
            pending_account_request: None,
            account_error: None,
            server_url,
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
            files_compact: false,
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

    fn begin_account_request(&mut self) {
        let requested_url = if self.account_form.remote {
            self.account_form.remote_url.clone()
        } else {
            self.local_server_url.clone()
        };
        let client = match ManagementClient::new(requested_url) {
            Ok(client) => client,
            Err(error) => {
                self.account_error = Some(error.to_string());
                return;
            }
        };
        let server = if self.account_form.remote {
            ServerLocation::Remote(client.url().to_owned())
        } else {
            ServerLocation::Local
        };
        let url = client.url().to_owned();
        let register = self.account_form.register;
        let email = self.account_form.email.clone();
        let display_name = self.account_form.display_name.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        thread::Builder::new()
            .name("block-account-request".into())
            .spawn(move || {
                let result = tokio::runtime::Runtime::new()
                    .map_err(|error| error.to_string())
                    .and_then(|runtime| {
                        runtime
                            .block_on(async move {
                                if register {
                                    client.register(email, display_name).await
                                } else {
                                    client.login(email).await
                                }
                            })
                            .map_err(|error| error.to_string())
                    });
                let _ = sender.send(result);
            })
            .unwrap_or_else(|error| panic!("failed to start account request: {error}"));
        self.account_error = None;
        self.pending_account_request = Some(PendingAccountRequest {
            receiver,
            server,
            url,
        });
    }

    fn poll_account_request(&mut self, ctx: &egui::Context) {
        let result = self
            .pending_account_request
            .as_ref()
            .and_then(|pending| pending.receiver.try_recv().ok());
        let Some(result) = result else {
            return;
        };
        let pending = self.pending_account_request.take().unwrap();
        match result {
            Ok(account) => {
                let saved = SavedAccount {
                    server: pending.server,
                    id: account.id,
                    email: account.email,
                    name: account.display_name,
                    last_workspace_id: None,
                };
                if let Err(error) = self.app_state.save_account(&saved) {
                    self.account_error = Some(error.to_string());
                    return;
                }
                self.accounts = self.app_state.accounts().unwrap_or_else(|error| {
                    self.account_error = Some(error.to_string());
                    Vec::new()
                });
                self.server_url = pending.url;
                self.account_form = AccountForm::default();
                self.switch_account(ctx, saved);
            }
            Err(error) => self.account_error = Some(error),
        }
    }

    fn show_account_onboarding(&mut self, ui: &mut egui::Ui) {
        self.poll_account_request(ui.ctx());
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.heading("Block Editor");
                ui.label("Choose an account or add one to get started.");
                ui.add_space(16.0);
            });

            let accounts = self.accounts.clone();
            for account in accounts {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.strong(&account.name);
                            ui.small(&account.email);
                            ui.weak(match &account.server {
                                ServerLocation::Local => "Local server",
                                ServerLocation::Remote(url) => url,
                            });
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Open").clicked() {
                                self.switch_account(ui.ctx(), account.clone());
                            }
                            if ui.button("Forget").clicked() {
                                if let Err(error) = self.app_state.remove_account(&account) {
                                    self.account_error = Some(error.to_string());
                                } else {
                                    self.accounts.retain(|saved| saved != &account);
                                }
                            }
                        });
                    });
                });
            }

            ui.add_space(16.0);
            ui.heading("Add account");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.account_form.remote, false, "Local server");
                ui.selectable_value(&mut self.account_form.remote, true, "Remote server");
            });
            if self.account_form.remote {
                ui.label("Server URL");
                ui.text_edit_singleline(&mut self.account_form.remote_url);
            }
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.account_form.register, false, "Log in");
                ui.selectable_value(&mut self.account_form.register, true, "Register");
            });
            ui.label("Email address");
            ui.text_edit_singleline(&mut self.account_form.email);
            if self.account_form.register {
                ui.label("Display name");
                ui.text_edit_singleline(&mut self.account_form.display_name);
            }
            if let Some(error) = &self.account_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            if self.pending_account_request.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Contacting server...");
                });
            } else if ui
                .add_enabled(
                    !self.account_form.email.trim().is_empty()
                        && (!self.account_form.register
                            || !self.account_form.display_name.trim().is_empty()),
                    egui::Button::new(if self.account_form.register {
                        "Register"
                    } else {
                        "Log in"
                    }),
                )
                .clicked()
            {
                self.begin_account_request();
            }
        });
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
        let server_url = match &account.server {
            ServerLocation::Local => self.local_server_url.clone(),
            ServerLocation::Remote(url) => url.clone(),
        };
        let client = BlockClient::new(account.id);
        client.connect(server_url.clone());
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
        self.files_compact = false;
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
        self.server_url = server_url;
        self.signed_in = true;
        if let Err(error) = self.app_state.set_active_account(&self.account) {
            self.account_error = Some(error.to_string());
        }
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
        let Some(action) = self.pending_destructive_action.clone() else {
            return;
        };
        let mut discard = false;
        let mut cancel = false;
        let (title, message, button) = match action {
            PendingDestructiveAction::Switch(ref account) => (
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

    fn handle_picker_result(&mut self, result: BlockPickerResult, target: BlockPickerTarget) {
        self.block_types.insert(result.id, result.block_type);
        match target {
            BlockPickerTarget::Root => self.open_tab(result.id, result.block_type),
            BlockPickerTarget::Block(parent) => self.queue_placement(
                BlockReference {
                    id: result.id,
                    block_type: result.block_type,
                    author: result.author,
                    name: result.name,
                    parent: BlockParent::Root,
                    references: 0,
                },
                parent,
            ),
        }
    }

    fn show_block_picker(&mut self, context: &egui::Context) {
        let target = self.block_picker_target.unwrap_or(BlockPickerTarget::Root);
        let parent = match target {
            BlockPickerTarget::Root => BlockParent::Root,
            BlockPickerTarget::Block(parent) => BlockParent::Uuid(parent),
        };
        let active = self.active_tab.unwrap_or(Uuid::nil());
        let result = {
            let mut editors =
                EditorAccess::new(active, &self.client, &self.registry, &mut self.editors);
            self.block_picker.handle(context, &mut editors, parent)
        };
        let Some(result) = result else {
            return;
        };
        self.block_picker_target = None;
        self.handle_picker_result(result, target);
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
                if let Some(child) = self.editors.get(&transfer.child.id) {
                    child.set_parent(parent);
                } else {
                    self.client.set_block_parent(transfer.child.id, parent);
                }
            }
        }
    }

    fn open_tab(&mut self, id: Uuid, block_type: Uuid) {
        self.ensure_block_open(id, block_type);
        let existing_tab = self.dock_state.iter_all_tabs().find_map(|(path, tab)| {
            matches!(tab, DockTab::Block(tab) if tab.current().id == id).then_some(path)
        });
        if let Some(path) = existing_tab {
            let _ = self.dock_state.set_active_tab(path);
            self.dock_state
                .set_focused_node_and_surface(path.node_path());
        } else {
            let tab = DockTab::Block(BlockTab::new(id, block_type));
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
                let _ = self.dock_state.set_active_tab(empty_path);
                self.dock_state
                    .set_focused_node_and_surface(empty_path.node_path());
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
        if self.active_tab != Some(id) {
            self.sidebar_reveal = None;
        }
        self.active_tab = Some(id);
    }

    fn ensure_block_open(&mut self, id: Uuid, block_type: Uuid) {
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
    }

    fn navigate_tab(&mut self, tab_id: Uuid, navigation: TabNavigation) {
        let destination = match navigation {
            TabNavigation::Open(item) => Some(item),
            TabNavigation::Back | TabNavigation::Forward => None,
        };
        if let Some(item) = destination {
            self.ensure_block_open(item.id, item.block_type);
        }
        let Some(tab) = self
            .dock_state
            .iter_all_tabs_mut()
            .find_map(|(_, tab)| match tab {
                DockTab::Block(tab) if tab.id == tab_id => Some(tab),
                DockTab::Files | DockTab::Empty | DockTab::Block(_) => None,
            })
        else {
            return;
        };
        match navigation {
            TabNavigation::Back => tab.go_back(),
            TabNavigation::Forward => tab.go_forward(),
            TabNavigation::Open(item) => tab.navigate(item),
        }
        let current = tab.current();
        self.ensure_block_open(current.id, current.block_type);
        self.active_tab = Some(current.id);
        self.sidebar_reveal = None;
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
        self.registry
            .icon_label(reference.block_type, &reference.name)
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
        let mut picker_excluded = path.clone();
        picker_excluded.insert(reference.id);
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
                                ICON_KEYBOARD_ARROW_DOWN
                            } else {
                                ICON_KEYBOARD_ARROW_RIGHT
                            }
                        } else if is_reference {
                            ICON_ARROW_FORWARD
                        } else {
                            ICON_CIRCLE
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
                match block_context_menu(
                    ui,
                    &self.registry,
                    &mut self.block_picker,
                    picker_excluded.clone(),
                    can_add_child,
                    can_delete_child,
                ) {
                    Some(BlockContextMenuAction::Picker) => {
                        self.block_picker_target =
                            Some(BlockPickerTarget::Block(reference.id));
                    }
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
                ui.menu_button(ICON_ADD, |ui| {
                    self.block_picker_target = Some(BlockPickerTarget::Block(reference.id));
                    self.block_picker.show_menu_excluding(
                        ui,
                        &self.registry,
                        picker_excluded.clone(),
                    );
                })
                .response
                .on_hover_text("Add a child");
            }
        });

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
        ui.horizontal(|ui| {
            ui.heading("Blocks");
            ui.add_space(ui.available_width() - 28.0);
            ui.menu_button(ICON_ADD, |ui| {
                self.block_picker_target = Some(BlockPickerTarget::Root);
                self.block_picker.show_menu(ui, &self.registry);
            })
            .response
            .on_hover_text("Create block");
        });
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
            (ICON_ARROW_UPWARD, viewport.top())
        } else if at_bottom {
            (
                egui_material_icons::icons::ICON_ARROW_DOWNWARD,
                viewport.bottom() - ui.spacing().interact_size.y,
            )
        } else if highlight.collapsed {
            (
                ICON_ARROW_UPWARD,
                viewport.bottom() - ui.spacing().interact_size.y,
            )
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
                egui::Button::new(format!("{} ({})", arrow.codepoint, active.name)),
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
                        ICON_KEYBOARD_ARROW_DOWN
                    } else {
                        ICON_KEYBOARD_ARROW_RIGHT
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
                ui.horizontal(|ui| {
                    ui.small(ICON_CHECK);
                    ui.small("All changes saved");
                });
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
                    if ui.button("Performance").clicked() {
                        performance::open();
                        ui.close();
                    }
                    ui.separator();
                    ui.strong("Accounts");
                    ui.small(format!("Signed in as {}", self.account.name));
                    ui.small(self.account.id.to_string());
                    for account in self.accounts.clone() {
                        if ui
                            .selectable_label(account == self.account, &account.name)
                            .on_hover_text(account.id.to_string())
                            .clicked()
                        {
                            self.request_account_switch(account);
                            ui.close();
                        }
                    }
                    if ui
                        .add_enabled(debug.changes_saved, egui::Button::new("Manage accounts"))
                        .on_disabled_hover_text("Wait for changes to finish saving")
                        .clicked()
                    {
                        self.signed_in = false;
                        if let Err(error) = self.app_state.clear_active_account() {
                            self.account_error = Some(error.to_string());
                        }
                        ui.close();
                    }
                });
            });
        });
    }

    fn show_content(
        &mut self,
        ui: &mut egui::Ui,
        active: Uuid,
        can_go_back: bool,
        can_go_forward: bool,
    ) -> (Option<EditorAction>, Option<TabNavigation>) {
        let Some(mut editor) = self.editors.remove(&active) else {
            return (None, None);
        };
        self.show_dynamic_artifact_bar(ui, active, editor.block_type());
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
        let block_type = editor.block_type();
        let current_name = editor.name();
        let relationships = editor.relationships();
        let mut navigation = None;
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(can_go_back, egui::Button::new(ICON_ARROW_BACK))
                .on_hover_text("Back")
                .clicked()
            {
                navigation = Some(TabNavigation::Back);
            }
            if ui
                .add_enabled(can_go_forward, egui::Button::new(ICON_ARROW_FORWARD))
                .on_hover_text("Forward")
                .clicked()
            {
                navigation = Some(TabNavigation::Forward);
            }
            ui.separator();
            let history = editor.history();
            if ui
                .add_enabled(
                    history.map_or_else(|| false, |history| history.can_undo()),
                    egui::Button::new(ICON_UNDO),
                )
                .on_hover_text("Undo (Ctrl/Cmd+Z)")
                .clicked()
                || undo_requested
            {
                if let Some(history) = history {
                    history.undo();
                }
            }
            if ui
                .add_enabled(
                    history.map_or_else(|| false, |history| history.can_redo()),
                    egui::Button::new(ICON_REDO),
                )
                .on_hover_text("Redo")
                .on_hover_text("Redo (Ctrl+Y or Ctrl/Cmd+Shift+Z)")
                .clicked()
                || redo_requested
            {
                if let Some(history) = history {
                    history.redo();
                }
            }
            ui.separator();
            if let Some(item) = self.show_breadcrumbs(
                ui,
                active,
                block_type,
                &current_name,
                relationships.as_ref(),
            ) {
                navigation = Some(TabNavigation::Open(item));
            }
        });
        ui.separator();
        let mut editors =
            EditorAccess::new(active, &self.client, &self.registry, &mut self.editors);
        let action = direct_editor_tab_ui(editor.as_mut(), ui, &mut editors);
        self.editors.insert(active, editor);
        (action, navigation)
    }

    fn show_breadcrumbs(
        &mut self,
        ui: &mut egui::Ui,
        active: Uuid,
        block_type: Uuid,
        current_name: &str,
        relationships: Option<&block_client::BlockRelationships>,
    ) -> Option<BlockTabHistoryItem> {
        let mut navigate = None;
        let parents =
            relationships.and_then(|_| self.parents.get(&active).map(ReferenceList::read));
        let root = parents
            .as_ref()
            .and_then(|parents| parents.first().map(|parent| parent.parent))
            .or_else(|| relationships.map(|relationships| relationships.parent));
        ui.label(match root {
            Some(BlockParent::Orphaned) => "Recently Deleted",
            Some(BlockParent::Root) => "Root",
            Some(BlockParent::Uuid(_)) | None => "Unknown",
        });
        if let Some(parents) = parents {
            for parent in parents {
                self.record_reference_types(&parent);
                ui.label(ICON_CHEVRON_RIGHT);
                if ui
                    .button(self.registry.icon_label(parent.block_type, &parent.name))
                    .on_hover_text(parent.id.to_string())
                    .clicked()
                {
                    navigate = Some(BlockTabHistoryItem {
                        id: parent.id,
                        block_type: parent.block_type,
                    });
                }
            }
        }
        ui.label(ICON_CHEVRON_RIGHT);
        ui.label(self.registry.icon_label(block_type, current_name));
        navigate
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
                        .add_enabled(!running, egui::Button::new(ICON_REFRESH))
                        .on_hover_text("Regenerate")
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

    fn handle_editor_action(&mut self, tab_id: Uuid, action: EditorAction) {
        match action {
            EditorAction::OpenBlock { id, block_type } => self.navigate_tab(
                tab_id,
                TabNavigation::Open(BlockTabHistoryItem { id, block_type }),
            ),
        }
    }

    fn show_dock(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let mut dock_state = std::mem::replace(&mut self.dock_state, default_dock_state());
        let files_compact = ui.available_width() < COMPACT_FILES_WIDTH;
        if files_compact != self.files_compact {
            set_files_compact(&mut dock_state, files_compact);
            self.files_compact = files_compact;
        }
        for editor in self.editors.values_mut() {
            editor.update(frame);
            editor.set_tab_active(false);
        }
        let mut viewer = BlockTabViewer {
            app: self,
            actions: Vec::new(),
            navigations: Vec::new(),
            tabs_to_close: Vec::new(),
        };
        DockArea::new(&mut dock_state).show_inside(ui, &mut viewer);
        for editor in viewer.app.editors.values_mut() {
            editor.finish_frame();
        }
        let tabs_to_close = std::mem::take(&mut viewer.tabs_to_close);
        for tab_id in tabs_to_close {
            let Some((path, current)) =
                dock_state
                    .iter_all_tabs()
                    .find_map(|(path, tab)| match tab {
                        DockTab::Block(tab) if tab.id == tab_id => Some((path, tab.current())),
                        DockTab::Files | DockTab::Empty | DockTab::Block(_) => None,
                    })
            else {
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
            viewer.app.close_tab_resources(current.id);
        }
        ensure_empty_workspace(&mut dock_state);
        let actions = std::mem::take(&mut viewer.actions);
        let pending_tabs = viewer
            .app
            .dock_state
            .iter_all_tabs()
            .filter_map(|(_, tab)| match tab {
                DockTab::Files | DockTab::Empty => None,
                DockTab::Block(tab) => Some(tab.current()),
            })
            .collect::<Vec<_>>();
        let previous_active = viewer.app.active_tab;
        let active_tab = dock_state
            .find_active_focused()
            .and_then(|(_, tab)| match tab {
                DockTab::Files | DockTab::Empty => None,
                DockTab::Block(tab) => Some(tab.current().id),
            })
            .or_else(|| {
                previous_active.filter(|id| {
                    dock_state.iter_all_tabs().any(
                        |(_, tab)| matches!(tab, DockTab::Block(tab) if tab.current().id == *id),
                    )
                })
            })
            .or_else(|| {
                dock_state.iter_all_tabs().find_map(|(_, tab)| match tab {
                    DockTab::Files | DockTab::Empty => None,
                    DockTab::Block(tab) => Some(tab.current().id),
                })
            });
        viewer.app.dock_state = dock_state;
        if previous_active != active_tab {
            viewer.app.sidebar_reveal = None;
        }
        viewer.app.active_tab = active_tab;
        for item in pending_tabs {
            viewer.app.open_tab(item.id, item.block_type);
        }
        for (tab_id, navigation) in std::mem::take(&mut viewer.navigations) {
            viewer.app.navigate_tab(tab_id, navigation);
        }
        for (tab_id, _, action) in actions {
            viewer.app.handle_editor_action(tab_id, action);
        }
    }

    fn show_statusbar(&mut self, ui: &mut egui::Ui, active: Uuid) -> Option<BlockTabHistoryItem> {
        let Some(editor) = self.editors.get(&active) else {
            return None;
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
        let mut navigate = None;
        let mut context_action = None;

        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Type: {type_name}"));
            ui.separator();
            let Some(relationships) = &relationships else {
                ui.label("Relationships loading…");
                return;
            };

            let Some(_parents) = &parents else {
                ui.label("Parents loading…");
                return;
            };
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
                BlockContextMenuAction::Picker => {
                    self.block_picker_target = Some(BlockPickerTarget::Block(reference.id));
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
        navigate.map(|(id, block_type)| BlockTabHistoryItem { id, block_type })
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
                .button(
                    self.registry
                        .icon_label(reference.block_type, &reference.name),
                )
                .on_hover_text(reference.id.to_string());
            if response.clicked() {
                *navigate = Some((reference.id, reference.block_type));
                ui.close();
            }
            let can_add = self.registry.can_add_child(reference.block_type);
            let can_delete = source != SidebarDragSource::Orphaned && self.can_delete_from(source);
            response.context_menu(|ui| {
                if let Some(action) = block_context_menu(
                    ui,
                    &self.registry,
                    &mut self.block_picker,
                    [reference.id],
                    can_add,
                    can_delete,
                ) {
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
    actions: Vec<(Uuid, Uuid, EditorAction)>,
    navigations: Vec<(Uuid, TabNavigation)>,
    tabs_to_close: Vec<Uuid>,
}

impl TabViewer for BlockTabViewer<'_> {
    type Tab = DockTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            DockTab::Files => "Files".into(),
            DockTab::Empty => "Workspace".into(),
            DockTab::Block(tab) => self
                .app
                .editors
                .get(&tab.current().id)
                .map_or_else(|| tab.current().id.to_string(), |editor| editor.name())
                .into(),
        }
    }

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        match tab {
            DockTab::Files => egui::Id::new("files-tab"),
            DockTab::Empty => egui::Id::new("empty-tab"),
            DockTab::Block(tab) => egui::Id::new(tab.id),
        }
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
            DockTab::Block(tab) => {
                let current = tab.current();
                if let Some(editor) = self.app.editors.get_mut(&current.id) {
                    editor.set_tab_active(true);
                }
                let mut status_navigation = None;
                egui::Panel::bottom(egui::Id::new(("block-statusbar", tab.id)))
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        status_navigation = self.app.show_statusbar(ui, current.id)
                    });
                if let Some(item) = status_navigation {
                    self.navigations.push((tab.id, TabNavigation::Open(item)));
                }
                let (action, navigation) =
                    self.app
                        .show_content(ui, current.id, tab.can_go_back(), tab.can_go_forward());
                if let Some(action) = action {
                    self.actions.push((tab.id, current.id, action));
                }
                if let Some(navigation) = navigation {
                    self.navigations.push((tab.id, navigation));
                }
            }
        }
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> OnCloseResponse {
        match tab {
            DockTab::Files | DockTab::Empty => OnCloseResponse::Ignore,
            DockTab::Block(tab) => {
                self.tabs_to_close.push(tab.id);
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
    Picker,
    Rename,
    Delete,
}

fn block_context_menu(
    ui: &mut egui::Ui,
    registry: &EditorRegistry,
    picker: &mut BlockPicker,
    excluded: impl IntoIterator<Item = Uuid>,
    can_add: bool,
    can_delete: bool,
) -> Option<BlockContextMenuAction> {
    let mut action = None;
    ui.add_enabled_ui(can_add, |ui| {
        ui.menu_button("Add", |ui| {
            picker.show_menu_excluding(ui, registry, excluded);
            action = Some(BlockContextMenuAction::Picker);
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
        performance::begin_frame();
        if !self.signed_in {
            self.show_account_onboarding(ui);
            performance::end_frame();
            ui.ctx().request_repaint_after(Duration::from_millis(100));
            return;
        }
        if let Some(account) = self.scheduled_account_switch.take() {
            self.switch_account(ui.ctx(), account);
        }
        self.intercept_close(ui.ctx());
        self.process_pending_transfers();
        self.show_block_picker(ui.ctx());
        self.show_rename(ui);
        self.show_client_debug(ui.ctx());
        self.show_network_debug(ui.ctx());

        self.show_dock(ui, frame);
        self.show_discard_confirmation(ui.ctx());
        performance::show(ui.ctx());
        performance::end_frame();
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
