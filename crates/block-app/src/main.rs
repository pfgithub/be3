mod app_state;
mod block_picker;
mod debug;
mod editors;
mod files;
mod performance;
mod platform;
mod share;

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    time::Duration,
};

#[cfg(not(target_arch = "wasm32"))]
use std::{io, path::PathBuf};

use app_state::{AppStateStore, SavedAccount, ServerLocation};
use block::{
    BlockAccess, BlockParent, BlockReference, BlockReferenceList, Workspace, WorkspaceInvitation,
    WorkspaceRole,
};
use block_client::{
    blocks::workspace_index::BlockEntry,
    presence::{pick_free_color, PresenceColor, UserActive},
    properties::MAX_NAME_BYTES,
    BlockClient, DynamicArtifactDescriptor, ManagementClient, ReferenceList, Session,
};
use block_picker::{BlockPicker, BlockPickerResult};
use editors::{
    direct_editor_tab_ui, BlockEditor, BlockLabel, DynamicArtifactRegeneration,
    DynamicArtifactSupport, EditorAccess, EditorAction, EditorRegistry, SidebarDragPayload,
    SidebarDragSource,
};
use eframe::egui;
use egui_dock::{widgets::tab_viewer::OnCloseResponse, DockArea, DockState, TabViewer};
use egui_material_icons::{
    icons::{
        ICON_ADD, ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_AUTO_AWESOME, ICON_CHEVRON_RIGHT,
        ICON_CLOUD, ICON_COMPUTER, ICON_CONTENT_COPY, ICON_DATA_OBJECT, ICON_EDIT, ICON_GROUP_ADD,
        ICON_KEYBOARD_ARROW_DOWN, ICON_LINK, ICON_LINK_OFF, ICON_LOCK, ICON_LOGOUT,
        ICON_MORE_HORIZ, ICON_REDO, ICON_REFRESH, ICON_SETTINGS, ICON_SHARE, ICON_SWITCH_ACCOUNT,
        ICON_UNDO, ICON_VISIBILITY, ICON_WORKSPACES,
    },
    MaterialIcon,
};
use share::ShareDialog;
use uuid::Uuid;

#[cfg(not(target_arch = "wasm32"))]
const APP_ID: &str = "Block";
const COMPACT_FILES_WIDTH: f32 = 700.0;
const NO_EDIT_ACCESS: &str = "You do not have permission to change this block";
/// How a block generated from another one is marked wherever it is listed.
const ICON_DYNAMIC_ARTIFACT: MaterialIcon = ICON_AUTO_AWESOME;
const ONBOARDING_WIDTH: f32 = 460.0;
/// The commit this build came from, stamped in by the build script.
const COMMIT: &str = env!("BLOCK_APP_COMMIT");
#[cfg(not(target_arch = "wasm32"))]
fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID)
            .with_inner_size([1100.0, 720.0]),
        ..Default::default()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_native(options: eframe::NativeOptions, storage_root: Option<PathBuf>) -> eframe::Result {
    eframe::run_native(
        APP_ID,
        options,
        Box::new(move |creation_context| {
            egui_material_icons::initialize(&creation_context.egui_ctx);
            editors::install_render_resources(creation_context);
            BlockApp::new(storage_root).map(|app| Box::new(app) as Box<dyn eframe::App>)
        }),
    )
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub fn run() -> eframe::Result {
    run_native(native_options(), None)
}

/// Starts the app on the canvas with the given element id. The page calls this
/// once the module is ready; it returns as soon as eframe owns the canvas, and
/// the app then runs from the browser's animation callbacks.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn run_web(canvas_id: String) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;

    // Panics otherwise reach only the WASI stderr shim, which is easy to miss.
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&format!("Block panicked: {info}").into());
    }));

    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("no browser document is available"))?;
    let canvas = document
        .get_element_by_id(&canvas_id)
        .ok_or_else(|| wasm_bindgen::JsValue::from_str(&format!("no element id {canvas_id}")))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|creation_context| {
                egui_material_icons::initialize(&creation_context.egui_ctx);
                editors::install_render_resources(creation_context);
                BlockApp::new()
                    .map(|app| Box::new(app) as Box<dyn eframe::App>)
                    .map_err(Into::into)
            }),
        )
        .await
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
    /// This installation's identity, used to pick out per-client settings
    /// entries. Generated once and persisted by `app_state`.
    client_id: Uuid,
    local_server_url: String,
    accounts: Vec<Account>,
    signed_in: bool,
    account_form: AccountForm,
    add_account_open: bool,
    pending_account_request: Option<PendingAccountRequest>,
    account_error: Option<String>,
    workspace: Option<Workspace>,
    workspaces: Vec<Workspace>,
    invitations: Vec<WorkspaceInvitation>,
    workspaces_loaded: bool,
    pending_workspace_request: Option<platform::RequestResult<Result<WorkspaceResult, String>>>,
    workspace_name: String,
    workspace_error: Option<String>,
    invite_open: bool,
    invite_email: String,
    invite_role: WorkspaceRole,
    scheduled_workspace_list: bool,
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
    /// Backrefs fetched on demand for the "Set parent" menu, keyed by the
    /// block whose parent is being changed. Unlike `backrefs`, this covers
    /// any block a context menu was opened on, not just open tabs.
    parent_candidates: HashMap<Uuid, ReferenceList>,
    block_types: HashMap<Uuid, Uuid>,
    registry: EditorRegistry,
    editors: HashMap<Uuid, Box<dyn BlockEditor>>,
    /// The access a tab is being shown at, when it is not the most its account
    /// has. Lets someone with edit access see what a viewer would see.
    editor_access: HashMap<Uuid, BlockAccess>,
    /// Tabs currently showing their block's raw serialized data instead of
    /// its editor.
    debug_tabs: HashSet<Uuid>,
    dynamic_artifact_regenerations: HashMap<Uuid, Box<dyn DynamicArtifactRegeneration>>,
    dynamic_artifact_errors: HashMap<Uuid, String>,
    /// Settings being edited in an artifact bar, until they are applied.
    dynamic_artifact_settings: HashMap<Uuid, Vec<u8>>,
    /// The block whose artifact settings modal is open, if any.
    dynamic_artifact_settings_open: Option<Uuid>,
    /// The block whose unlink confirmation is open, if any.
    dynamic_artifact_unlink: Option<Uuid>,
    dock_state: DockState<DockTab>,
    files_compact: bool,
    active_tab: Option<Uuid>,
    /// Blocks currently announced as [`UserActive`] presence, i.e. whose tab
    /// was on screen as of the last frame. Kept in sync by
    /// [`BlockApp::update_active_presence`].
    active_presence: HashSet<Uuid>,
    sidebar_reveal: Option<Uuid>,
    /// The immediate container a block was last opened through via an
    /// explicit sidebar tree click, keyed by the block id. Not persisted;
    /// used only to reveal the exact row a click came from instead of always
    /// falling back to the block's canonical ancestor chain.
    opened_via: HashMap<Uuid, Uuid>,
    pending_transfers: Vec<PendingTransfer>,
    pending_copies: Vec<PendingCopy>,
    rename: Option<RenameState>,
    share: ShareDialog,
    client_debug_open: bool,
    network_debug_open: bool,
    about_open: bool,
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

struct AccountForm {
    remote: bool,
    remote_url: String,
    register: bool,
    email: String,
    display_name: String,
    password: String,
}

impl Default for AccountForm {
    fn default() -> Self {
        Self {
            // A build with no embedded server can only reach a remote one.
            remote: !platform::HAS_EMBEDDED_SERVER,
            remote_url: String::new(),
            register: false,
            email: String::new(),
            display_name: String::new(),
            password: String::new(),
        }
    }
}

struct PendingAccountRequest {
    receiver: platform::RequestResult<Result<Session, String>>,
    server: ServerLocation,
    url: String,
}

enum WorkspaceOperation {
    Load,
    Create(String),
    Respond(Uuid, bool),
    Invite(Uuid, String, WorkspaceRole),
}

enum WorkspaceResult {
    Loaded(Vec<Workspace>, Vec<WorkspaceInvitation>),
    Created(Workspace),
    Responded,
    Invited,
}

#[derive(Clone)]
enum PendingDestructiveAction {
    Switch(Account),
    ChooseWorkspace,
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

/// Duplicates the block being viewed via a reference and swaps the specific
/// occurrence being viewed (in `container`) to point at the copy, without
/// touching the original block or its other referrers.
#[derive(Clone)]
struct PendingCopy {
    source: Uuid,
    container: Uuid,
    tab_id: Uuid,
    stage: CopyStage,
}

#[derive(Clone, Copy)]
enum CopyStage {
    Duplicate,
    Replace { copy_id: Uuid, block_type: Uuid },
}

struct RenameState {
    id: Uuid,
    name: String,
}

#[derive(Clone, Copy)]
enum BlockPickerTarget {
    Root,
    Block(Uuid),
}

impl BlockApp {
    /// Opens the app state beside the app and starts the block server that
    /// backs local accounts.
    #[cfg(not(target_arch = "wasm32"))]
    fn new(storage_root: Option<PathBuf>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let data_dir = storage_root
            .or_else(|| eframe::storage_dir(APP_ID))
            .ok_or_else(|| io::Error::other("application-data directory is unavailable"))?;
        std::fs::create_dir_all(&data_dir)?;
        let url = platform::start_embedded_server(data_dir.join("server"))?;
        let app_state = AppStateStore::open(data_dir.join("app.sqlite3"))?;
        Self::with_state(app_state, url)
    }

    /// Opens the app state in browser storage. There is no local server to
    /// start, so every account is a remote one.
    #[cfg(target_arch = "wasm32")]
    fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let app_state = AppStateStore::open()?;
        Self::with_state(app_state, String::new())
    }

    /// Restores the last session from `app_state`. `local_server_url` is where
    /// local accounts are served, and is empty where there are none.
    fn with_state(
        app_state: AppStateStore,
        url: String,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let client_id = app_state.client_id()?;
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
            token: String::new(),
            last_workspace_id: None,
        });
        let server_url = match &account.server {
            ServerLocation::Local => url.clone(),
            ServerLocation::Remote(remote) => remote.clone(),
        };
        let client = BlockClient::new(account.id, Uuid::nil());
        let roots = client.watch_references(BlockReferenceList::Roots);
        Ok(Self {
            app_state,
            client_id,
            local_server_url: url,
            accounts,
            signed_in,
            account_form: AccountForm::default(),
            add_account_open: false,
            pending_account_request: None,
            account_error: None,
            workspace: None,
            workspaces: Vec::new(),
            invitations: Vec::new(),
            workspaces_loaded: false,
            pending_workspace_request: None,
            workspace_name: String::new(),
            workspace_error: None,
            invite_open: false,
            invite_email: String::new(),
            invite_role: WorkspaceRole::Editor,
            scheduled_workspace_list: false,
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
            parent_candidates: HashMap::new(),
            block_types: HashMap::new(),
            registry: EditorRegistry::new(),
            editors: HashMap::new(),
            editor_access: HashMap::new(),
            debug_tabs: HashSet::new(),
            dynamic_artifact_regenerations: HashMap::new(),
            dynamic_artifact_errors: HashMap::new(),
            dynamic_artifact_settings: HashMap::new(),
            dynamic_artifact_settings_open: None,
            dynamic_artifact_unlink: None,
            dock_state: default_dock_state(),
            files_compact: false,
            active_tab: None,
            active_presence: HashSet::new(),
            sidebar_reveal: None,
            opened_via: HashMap::new(),
            pending_transfers: Vec::new(),
            pending_copies: Vec::new(),
            rename: None,
            share: ShareDialog::default(),
            client_debug_open: false,
            network_debug_open: false,
            about_open: false,
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
        let password = self.account_form.password.clone();
        let receiver = platform::spawn_request(async move {
            if register {
                client.register(email, display_name, password).await
            } else {
                client.login(email, password).await
            }
            .map_err(|error| error.to_string())
        });
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
            Ok(session) => {
                let saved = SavedAccount {
                    server: pending.server,
                    id: session.account.id,
                    email: session.account.email,
                    name: session.account.display_name,
                    token: session.token,
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
                self.add_account_open = false;
                self.switch_account(ctx, saved);
            }
            Err(error) => self.account_error = Some(error),
        }
    }

    fn show_account_onboarding(&mut self, ui: &mut egui::Ui) {
        self.poll_account_request(ui.ctx());
        let mut action = None;
        let mut add_account = false;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            onboarding_column(ui, |ui| {
                ui.add_space(36.0);
                ui.heading("Block Editor");
                ui.weak("Choose an account to continue.");
                ui.add_space(20.0);

                for account in &self.accounts {
                    if let Some(chosen) = show_account_card(ui, account) {
                        action = Some(chosen);
                    }
                    ui.add_space(8.0);
                }
                if self.accounts.is_empty() {
                    onboarding_card(ui, |ui| {
                        ui.weak("No accounts yet. Add one to get started.");
                    });
                    ui.add_space(8.0);
                }

                add_account = ui
                    .add_sized(
                        [ui.available_width(), 30.0],
                        egui::Button::new(format!("{} Add account", ICON_ADD.codepoint)),
                    )
                    .clicked();

                if !self.add_account_open {
                    if let Some(error) = &self.account_error {
                        ui.add_space(12.0);
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                }
                ui.add_space(24.0);
            });
        });

        if add_account {
            self.account_form = AccountForm::default();
            self.account_error = None;
            self.add_account_open = true;
        }
        match action {
            Some(AccountAction::Open(account)) => self.open_account(ui.ctx(), account, false),
            Some(AccountAction::ChooseWorkspace(account)) => {
                self.open_account(ui.ctx(), account, true);
            }
            Some(AccountAction::LogOut(account)) => self.log_out_account(&account),
            None => {}
        }
        self.show_add_account(ui.ctx());
    }

    fn open_account(&mut self, ctx: &egui::Context, mut account: Account, choose_workspace: bool) {
        if choose_workspace {
            account.last_workspace_id = None;
            if let Err(error) = self.app_state.set_last_workspace(&account, None) {
                self.account_error = Some(error.to_string());
            }
        }
        self.switch_account(ctx, account);
    }

    fn log_out_account(&mut self, account: &Account) {
        // Best-effort: revokes the token server-side so a lost or stolen
        // device cannot keep using it. The local sign-out below happens
        // either way, since that's the part the user actually asked for.
        let url = match &account.server {
            ServerLocation::Local => self.local_server_url.clone(),
            ServerLocation::Remote(url) => url.clone(),
        };
        let token = account.token.clone();
        let _ = platform::spawn_request(async move {
            if let Ok(client) = ManagementClient::new(url) {
                let _ = client.logout(token).await;
            }
        });

        if let Err(error) = self.app_state.remove_account(account) {
            self.account_error = Some(error.to_string());
            return;
        }
        self.accounts
            .retain(|saved| saved.server != account.server || saved.id != account.id);
        if self.account.server == account.server && self.account.id == account.id {
            self.signed_in = false;
        }
    }

    fn show_add_account(&mut self, ctx: &egui::Context) {
        if !self.add_account_open {
            return;
        }
        let mut close = false;
        let pending = self.pending_account_request.is_some();
        let ready = !pending
            && !self.account_form.email.trim().is_empty()
            && !self.account_form.password.is_empty()
            && (!self.account_form.register || !self.account_form.display_name.trim().is_empty());
        let response = egui::Modal::new(egui::Id::new("add-account")).show(ctx, |ui| {
            ui.set_width(320.0);
            ui.heading("Add account");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.account_form.register, false, "Log in");
                ui.selectable_value(&mut self.account_form.register, true, "Register");
            });
            ui.add_space(12.0);
            ui.label("Server");
            // Without an embedded server there is nothing for a local account to
            // talk to, so the choice is not offered and the URL is required.
            if platform::HAS_EMBEDDED_SERVER {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.account_form.remote, false, "Local");
                    ui.selectable_value(&mut self.account_form.remote, true, "Remote");
                });
            }
            if self.account_form.remote {
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.account_form.remote_url)
                        .hint_text("https://example.com")
                        .desired_width(f32::INFINITY),
                );
            }
            ui.add_space(12.0);
            ui.label("Email address");
            ui.add(
                egui::TextEdit::singleline(&mut self.account_form.email)
                    .hint_text("you@example.com")
                    .desired_width(f32::INFINITY),
            );
            if self.account_form.register {
                ui.add_space(12.0);
                ui.label("Display name");
                ui.add(
                    egui::TextEdit::singleline(&mut self.account_form.display_name)
                        .desired_width(f32::INFINITY),
                );
            }
            ui.add_space(12.0);
            ui.label("Password");
            ui.add(
                egui::TextEdit::singleline(&mut self.account_form.password)
                    .password(true)
                    .desired_width(f32::INFINITY),
            );
            if let Some(error) = &self.account_error {
                ui.add_space(8.0);
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.add_space(16.0);
            let mut submit = false;
            egui::Sides::new().show(
                ui,
                |ui| {
                    if pending {
                        ui.spinner();
                        ui.weak("Contacting server\u{2026}");
                    }
                },
                |ui| {
                    submit = ui
                        .add_enabled(
                            ready,
                            egui::Button::new(if self.account_form.register {
                                "Register"
                            } else {
                                "Log in"
                            })
                            .selected(ready),
                        )
                        .clicked();
                    close |= ui.button("Cancel").clicked();
                },
            );
            submit
        });
        if response.inner {
            self.begin_account_request();
            return;
        }
        if close || response.should_close() {
            self.add_account_open = false;
            self.pending_account_request = None;
            self.account_error = None;
        }
    }

    fn begin_workspace_request(&mut self, operation: WorkspaceOperation) {
        if self.pending_workspace_request.is_some() {
            return;
        }
        let client = match ManagementClient::new(self.server_url.clone()) {
            Ok(client) => client,
            Err(error) => {
                self.workspace_error = Some(error.to_string());
                return;
            }
        };
        let token = self.account.token.clone();
        let receiver = platform::spawn_request(async move {
            match operation {
                WorkspaceOperation::Load => {
                    let workspaces = client
                        .list_workspaces(&token)
                        .await
                        .map_err(|error| error.to_string())?;
                    let invitations = client
                        .list_invitations(&token)
                        .await
                        .map_err(|error| error.to_string())?;
                    Ok(WorkspaceResult::Loaded(workspaces, invitations))
                }
                WorkspaceOperation::Create(name) => client
                    .create_workspace(&token, name)
                    .await
                    .map(WorkspaceResult::Created)
                    .map_err(|error| error.to_string()),
                WorkspaceOperation::Respond(invitation_id, accept) => client
                    .respond_invitation(&token, invitation_id, accept)
                    .await
                    .map(|()| WorkspaceResult::Responded)
                    .map_err(|error| error.to_string()),
                WorkspaceOperation::Invite(workspace_id, email, role) => client
                    .invite(&token, workspace_id, email, role)
                    .await
                    .map(|_| WorkspaceResult::Invited)
                    .map_err(|error| error.to_string()),
            }
        });
        self.workspace_error = None;
        self.pending_workspace_request = Some(receiver);
    }

    fn poll_workspace_request(&mut self) {
        let result = self
            .pending_workspace_request
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok());
        let Some(result) = result else {
            return;
        };
        self.pending_workspace_request = None;
        match result {
            Ok(WorkspaceResult::Loaded(workspaces, invitations)) => {
                self.workspaces = workspaces;
                self.invitations = invitations;
                self.workspaces_loaded = true;
                if let Some(last_workspace_id) = self.account.last_workspace_id {
                    if let Some(workspace) = self
                        .workspaces
                        .iter()
                        .find(|workspace| workspace.id == last_workspace_id)
                        .cloned()
                    {
                        self.open_workspace(workspace);
                    }
                }
            }
            Ok(WorkspaceResult::Created(workspace)) => {
                self.workspaces.push(workspace.clone());
                self.workspace_name.clear();
                self.open_workspace(workspace);
            }
            Ok(WorkspaceResult::Responded) => {
                self.workspaces_loaded = false;
                self.begin_workspace_request(WorkspaceOperation::Load);
            }
            Ok(WorkspaceResult::Invited) => {
                self.invite_email.clear();
                self.invite_open = false;
            }
            Err(error) => self.workspace_error = Some(error),
        }
    }

    fn open_workspace(&mut self, workspace: Workspace) {
        let client = BlockClient::new(self.account.id, workspace.id);
        client.connect(self.server_url.clone(), self.account.token.clone());
        let roots = client.watch_references(BlockReferenceList::Roots);
        self.orphaned = None;
        self.orphaned_expanded = false;
        self.expanded.clear();
        self.parents.clear();
        self.references.clear();
        self.backrefs.clear();
        self.block_types.clear();
        self.opened_via.clear();
        self.registry = EditorRegistry::new();
        self.editors.clear();
        self.dynamic_artifact_regenerations.clear();
        self.dynamic_artifact_errors.clear();
        self.dynamic_artifact_settings.clear();
        self.dynamic_artifact_settings_open = None;
        self.dynamic_artifact_unlink = None;
        self.dock_state = default_dock_state();
        self.active_tab = None;
        self.share = ShareDialog::default();
        self.roots = roots;
        self.client = client;
        self.workspace = Some(workspace.clone());
        self.account.last_workspace_id = Some(workspace.id);
        if let Some(saved) = self
            .accounts
            .iter_mut()
            .find(|saved| saved.server == self.account.server && saved.id == self.account.id)
        {
            saved.last_workspace_id = Some(workspace.id);
        }
        if let Err(error) = self
            .app_state
            .set_last_workspace(&self.account, Some(workspace.id))
        {
            self.workspace_error = Some(error.to_string());
        }
    }

    fn show_workspace_onboarding(&mut self, ui: &mut egui::Ui) {
        if !self.workspaces_loaded && self.pending_workspace_request.is_none() {
            self.begin_workspace_request(WorkspaceOperation::Load);
        }
        self.poll_workspace_request();
        let busy = self.pending_workspace_request.is_some();
        let can_create = !busy && !self.workspace_name.trim().is_empty();
        let mut open_workspace = None;
        let mut respond = None;
        let mut create = false;
        let mut refresh = false;
        let mut switch_account = false;
        let mut log_out = false;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            onboarding_column(ui, |ui| {
                ui.add_space(36.0);
                egui::Sides::new().shrink_left().show(
                    ui,
                    |ui| {
                        ui.heading("Workspaces");
                    },
                    |ui| {
                        ui.menu_button(ICON_MORE_HORIZ, |ui| {
                            if ui
                                .button(format!("{} Log out", ICON_LOGOUT.codepoint))
                                .clicked()
                            {
                                log_out = true;
                                ui.close();
                            }
                        })
                        .response
                        .on_hover_text("Account options");
                        refresh = ui
                            .add_enabled(!busy, egui::Button::new(ICON_REFRESH))
                            .on_hover_text("Reload workspaces")
                            .clicked();
                    },
                );
                ui.add_space(12.0);

                onboarding_card(ui, |ui| {
                    egui::Sides::new().shrink_left().show(
                        ui,
                        |ui| {
                            ui.vertical(|ui| {
                                account_name(ui, &self.account);
                                account_details(ui, &self.account);
                            });
                        },
                        |ui| {
                            switch_account = ui
                                .button(format!("{} Switch account", ICON_SWITCH_ACCOUNT.codepoint))
                                .clicked();
                        },
                    );
                });

                ui.add_space(20.0);
                ui.strong("Open a workspace");
                ui.add_space(6.0);
                for workspace in &self.workspaces {
                    if ui
                        .add_sized(
                            [ui.available_width(), 32.0],
                            egui::Button::new(format!(
                                "{} {}",
                                ICON_WORKSPACES.codepoint, workspace.name
                            ))
                            .right_text(ICON_CHEVRON_RIGHT)
                            .truncate(),
                        )
                        .clicked()
                    {
                        open_workspace = Some(workspace.clone());
                    }
                    ui.add_space(4.0);
                }
                if self.workspaces.is_empty() {
                    onboarding_card(ui, |ui| {
                        if self.workspaces_loaded {
                            ui.weak("You do not have any workspaces yet. Create one below.");
                        } else {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.weak("Loading workspaces\u{2026}");
                            });
                        }
                    });
                }

                if !self.invitations.is_empty() {
                    ui.add_space(20.0);
                    ui.strong(format!("{} Invitations", ICON_GROUP_ADD.codepoint));
                    ui.add_space(6.0);
                    for invitation in &self.invitations {
                        onboarding_card(ui, |ui| {
                            egui::Sides::new().shrink_left().show(
                                ui,
                                |ui| {
                                    ui.vertical(|ui| {
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(
                                                    invitation.workspace_name.as_str(),
                                                )
                                                .strong(),
                                            )
                                            .truncate(),
                                        );
                                        ui.small(format!(
                                            "Invited as {}",
                                            invitation.role.label().to_lowercase()
                                        ));
                                    });
                                },
                                |ui| {
                                    if ui
                                        .add_enabled(
                                            !busy,
                                            egui::Button::new("Accept").selected(!busy),
                                        )
                                        .clicked()
                                    {
                                        respond = Some((invitation.id, true));
                                    }
                                    if ui
                                        .add_enabled(!busy, egui::Button::new("Decline"))
                                        .clicked()
                                    {
                                        respond = Some((invitation.id, false));
                                    }
                                },
                            );
                        });
                        ui.add_space(4.0);
                    }
                }

                ui.add_space(20.0);
                ui.strong("Create a workspace");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let button_width = 76.0;
                    let field_width =
                        (ui.available_width() - button_width - ui.spacing().item_spacing.x)
                            .max(80.0);
                    let response = ui.add_sized(
                        [field_width, 26.0],
                        egui::TextEdit::singleline(&mut self.workspace_name)
                            .hint_text("Workspace name"),
                    );
                    let submitted = response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    let clicked = ui
                        .add_enabled_ui(can_create, |ui| {
                            ui.add_sized(
                                [button_width, 26.0],
                                egui::Button::new("Create").selected(can_create),
                            )
                            .clicked()
                        })
                        .inner;
                    create = can_create && (clicked || submitted);
                });

                if let Some(error) = &self.workspace_error {
                    ui.add_space(12.0);
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.add_space(24.0);
            });
        });

        if let Some(workspace) = open_workspace {
            self.open_workspace(workspace);
        }
        if let Some((invitation, accept)) = respond {
            self.begin_workspace_request(WorkspaceOperation::Respond(invitation, accept));
        }
        if create {
            self.begin_workspace_request(WorkspaceOperation::Create(self.workspace_name.clone()));
        }
        if refresh {
            self.workspaces_loaded = false;
            self.begin_workspace_request(WorkspaceOperation::Load);
        }
        if switch_account {
            self.signed_in = false;
            if let Err(error) = self.app_state.clear_active_account() {
                self.account_error = Some(error.to_string());
            }
        }
        if log_out {
            let account = self.account.clone();
            self.log_out_account(&account);
        }
    }

    fn show_invite(&mut self, ctx: &egui::Context) {
        if !self.invite_open {
            return;
        }
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        let mut open = self.invite_open;
        egui::Window::new("Invite member")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("Workspace: {}", workspace.name));
                ui.label("Email address");
                ui.text_edit_singleline(&mut self.invite_email);
                ui.add_space(8.0);
                ui.label("Role");
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.invite_role,
                        WorkspaceRole::Editor,
                        WorkspaceRole::Editor.label(),
                    );
                    ui.selectable_value(
                        &mut self.invite_role,
                        WorkspaceRole::Administrator,
                        WorkspaceRole::Administrator.label(),
                    );
                });
                ui.small(match self.invite_role {
                    WorkspaceRole::Administrator => "Can open every block in the workspace.",
                    WorkspaceRole::Editor => {
                        "Can only open blocks they create or are given access to."
                    }
                });
                ui.add_space(8.0);
                if ui
                    .add_enabled(
                        !self.invite_email.trim().is_empty()
                            && self.pending_workspace_request.is_none(),
                        egui::Button::new("Send invitation"),
                    )
                    .clicked()
                {
                    self.begin_workspace_request(WorkspaceOperation::Invite(
                        workspace.id,
                        self.invite_email.clone(),
                        self.invite_role,
                    ));
                }
                if let Some(error) = &self.workspace_error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });
        self.invite_open = open;
    }

    fn show_about(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let mut open = self.about_open;
        egui::Window::new("About")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.strong("Block");
                ui.add_space(8.0);
                egui::Grid::new("about-build")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Version");
                        ui.monospace(env!("CARGO_PKG_VERSION"));
                        ui.end_row();
                        ui.label("Commit");
                        // Selectable so the hash can be copied into a bug report.
                        ui.add(
                            egui::Label::new(egui::RichText::new(COMMIT).monospace())
                                .selectable(true),
                        );
                        ui.end_row();
                    });
            });
        self.about_open = open;
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
        let client = BlockClient::new(account.id, Uuid::nil());
        let roots = client.watch_references(BlockReferenceList::Roots);

        self.orphaned = None;
        self.orphaned_expanded = false;
        self.expanded.clear();
        self.parents.clear();
        self.references.clear();
        self.backrefs.clear();
        self.block_types.clear();
        self.opened_via.clear();
        self.registry = EditorRegistry::new();
        self.editors.clear();
        self.dynamic_artifact_regenerations.clear();
        self.dynamic_artifact_errors.clear();
        self.dynamic_artifact_settings.clear();
        self.dynamic_artifact_settings_open = None;
        self.dynamic_artifact_unlink = None;
        self.dock_state = default_dock_state();
        self.files_compact = false;
        self.active_tab = None;
        self.sidebar_reveal = None;
        self.pending_transfers.clear();
        self.rename = None;
        self.share = ShareDialog::default();
        self.client_debug_open = false;
        self.network_debug_open = false;
        self.about_open = false;
        self.block_picker = BlockPicker::default();
        self.block_picker_target = None;
        self.pending_destructive_action = None;
        self.scheduled_account_switch = None;
        self.allow_close = false;
        self.workspace = None;
        self.workspaces.clear();
        self.invitations.clear();
        self.workspaces_loaded = false;
        self.pending_workspace_request = None;
        self.workspace_error = None;
        self.invite_open = false;
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
            PendingDestructiveAction::ChooseWorkspace => (
                "Discard unsaved changes?",
                "Switching workspaces will discard changes that have not reached the server."
                    .into(),
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
                PendingDestructiveAction::ChooseWorkspace => {
                    self.scheduled_workspace_list = true;
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
                    properties: result.properties,
                    parent: BlockParent::Orphaned,
                    references: 0,
                    // The picker only creates blocks from scratch.
                    dynamic_artifact: false,
                    // The account just made the block, so it is theirs.
                    access: BlockAccess::Edit,
                },
                parent,
            ),
        }
    }

    fn show_block_picker(&mut self, context: &egui::Context) {
        let target = self.block_picker_target.unwrap_or(BlockPickerTarget::Root);
        let parent = match target {
            BlockPickerTarget::Root => BlockParent::Root,
            // The server rejects a parent that does not reference the child,
            // and the parent only takes the reference once its editor has
            // loaded, which can be several frames later. Leave the block
            // orphaned; the queued placement sets the parent after that.
            BlockPickerTarget::Block(_) => BlockParent::Orphaned,
        };
        let active = self.active_tab.unwrap_or(Uuid::nil());
        let access = self.editor_access(active);
        let result = {
            let mut editors = EditorAccess::new(
                active,
                access,
                &self.client,
                self.client_id,
                &self.registry,
                &mut self.editors,
            );
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

    /// Sets a block's designated parent, going through its open editor if it
    /// has one so the change is reflected optimistically like other edits.
    fn set_block_parent(&mut self, id: Uuid, parent: BlockParent) {
        if let Some(editor) = self.editors.get(&id) {
            editor.set_parent(parent);
        } else {
            self.client.set_block_parent(id, parent);
        }
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
                self.set_block_parent(transfer.child.id, parent);
            }
        }
    }

    /// Queues duplicating `source` and swapping the occurrence viewed through
    /// `container` (the tab identified by `tab_id`) to point at the copy.
    fn queue_copy(&mut self, source: Uuid, container: Uuid, tab_id: Uuid) {
        if self
            .pending_copies
            .iter()
            .any(|pending| pending.tab_id == tab_id)
        {
            return;
        }
        self.ensure_editor(container);
        self.pending_copies.push(PendingCopy {
            source,
            container,
            tab_id,
            stage: CopyStage::Duplicate,
        });
    }

    fn process_pending_copies(&mut self) {
        let pending = std::mem::take(&mut self.pending_copies);
        for mut copy in pending {
            if let CopyStage::Duplicate = copy.stage {
                let Some((copy_id, block_type)) =
                    self.editors.get(&copy.source).and_then(|editor| {
                        editor
                            .block()
                            .duplicate(&self.client)
                            .map(|copy_id| (copy_id, editor.block_type()))
                    })
                else {
                    self.pending_copies.push(copy);
                    continue;
                };
                copy.stage = CopyStage::Replace {
                    copy_id,
                    block_type,
                };
            }
            let CopyStage::Replace {
                copy_id,
                block_type,
            } = copy.stage
            else {
                unreachable!("copy stage was just set to Replace")
            };

            if !self.ensure_editor(copy.container) {
                self.pending_copies.push(copy);
                continue;
            }
            let replaced = self
                .editors
                .get(&copy.container)
                .and_then(|editor| editor.replace_child(copy.source, BlockEntry { id: copy_id }));
            if replaced != Some(true) {
                self.pending_copies.push(copy);
                continue;
            }

            self.set_block_parent(copy_id, BlockParent::Uuid(copy.container));
            self.navigate_tab(
                copy.tab_id,
                TabNavigation::Open(BlockTabHistoryItem {
                    id: copy_id,
                    block_type,
                }),
            );
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
        self.editor_access.remove(&id);
        self.debug_tabs.remove(&id);
        self.parents.remove(&id);
        self.references.remove(&id);
        self.backrefs.remove(&id);
        self.dynamic_artifact_regenerations.remove(&id);
        self.dynamic_artifact_errors.remove(&id);
        self.forget_dynamic_artifact_dialogs(id);
        if let Some(editor) = self.editors.get_mut(&id) {
            editor.tab_closed();
        }
        if self.active_tab == Some(id) {
            self.active_tab = None;
            self.sidebar_reveal = None;
        }
    }

    /// The most a tab may do with its block, which is what its mode dropdown is
    /// locked to.
    fn editor_access_ceiling(&self, id: Uuid) -> BlockAccess {
        editors::editor_access_ceiling(&self.client, id)
    }

    /// The access a tab is being shown at: the mode it was put in, never above
    /// what the account is allowed.
    fn editor_access(&self, id: Uuid) -> BlockAccess {
        let ceiling = self.editor_access_ceiling(id);
        self.editor_access
            .get(&id)
            .map_or(ceiling, |chosen| (*chosen).min(ceiling))
    }

    fn show_content(
        &mut self,
        ui: &mut egui::Ui,
        tab_id: Uuid,
        active: Uuid,
        can_go_back: bool,
        can_go_forward: bool,
    ) -> (Option<EditorAction>, Option<TabNavigation>) {
        let Some(mut editor) = self.editors.remove(&active) else {
            return (None, None);
        };
        let access = self.editor_access(active);
        let denied = !access.can_view();
        let relationships = editor.relationships();
        let artifact_navigation = if denied {
            None
        } else {
            self.show_dynamic_artifact_bar(ui, active, editor.block_type())
        };
        let shared_navigation = if denied {
            None
        } else {
            self.show_shared_block_bar(ui, tab_id, active, relationships.as_ref())
        };
        let undo_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
        let redo_shortcut = egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        );
        let redo_y_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Y);
        let redo_requested = access.can_edit()
            && (ui
                .ctx()
                .input_mut(|input| input.consume_shortcut(&redo_shortcut))
                || ui
                    .ctx()
                    .input_mut(|input| input.consume_shortcut(&redo_y_shortcut)));
        let undo_requested = access.can_edit()
            && ui
                .ctx()
                .input_mut(|input| input.consume_shortcut(&undo_shortcut));
        let current_name = BlockLabel::for_handle(&self.registry, editor.block());
        let mut navigation = artifact_navigation
            .or(shared_navigation)
            .map(TabNavigation::Open);
        let mut share = false;
        let can_share = self.client.block_access(active).can_edit();
        let ceiling = self.editor_access_ceiling(active);
        let generated = self.client.is_dynamic_artifact(active);
        let debug = self.debug_tabs.contains(&active);
        let mut mode = if debug {
            TabMode::Debug
        } else {
            TabMode::Access(access)
        };
        egui::Sides::new().shrink_left().show(
            ui,
            |ui| {
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
                    let history = editor.history().filter(|_| access.can_edit());
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
                    if let Some(item) =
                        self.show_breadcrumbs(ui, active, &current_name, relationships.as_ref())
                    {
                        navigation = Some(TabNavigation::Open(item));
                    }
                });
            },
            |ui| {
                share = ui
                    .add_enabled(
                        can_share,
                        egui::Button::new(format!("{} Share", ICON_SHARE.codepoint)),
                    )
                    .on_hover_text("Share this block")
                    .on_disabled_hover_text("Only accounts that can edit a block may share it")
                    .clicked();
                mode = show_access_mode(ui, active, access, ceiling, generated, debug);
            },
        );
        if share {
            self.share.open(&self.client, active, current_name);
        }
        match mode {
            TabMode::Access(chosen_access) => {
                self.debug_tabs.remove(&active);
                if chosen_access != access {
                    self.editor_access.insert(active, chosen_access);
                }
            }
            TabMode::Debug => {
                self.debug_tabs.insert(active);
            }
        }
        ui.separator();
        if self.debug_tabs.contains(&active) {
            if ceiling.can_view() {
                self.editors.insert(active, editor);
                self.show_debug_data(ui, active);
                return (None, navigation);
            }
            self.debug_tabs.remove(&active);
        }
        if denied {
            self.editors.insert(active, editor);
            self.show_access_denied(ui, self.editor_access_ceiling(active).can_view());
            return (None, navigation);
        }
        let mut editors = EditorAccess::new(
            active,
            access,
            &self.client,
            self.client_id,
            &self.registry,
            &mut self.editors,
        );
        let action = direct_editor_tab_ui(editor.as_mut(), ui, &mut editors);
        self.editors.insert(active, editor);
        (action, navigation)
    }

    /// Replaces an editor whose block cannot be opened, either because the
    /// server refuses it or because the tab is showing it as an account that
    /// only knows it exists. The block is still listed either way, so the tab
    /// has to explain why it is empty.
    fn show_access_denied(&self, ui: &mut egui::Ui, simulated: bool) {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.heading(format!("{} No access", ICON_LOCK.codepoint));
                if simulated {
                    ui.weak("An account that only knows this block exists cannot open it.");
                    ui.weak("Switch back to Can view or Can edit to see it.");
                } else {
                    ui.weak("You do not have permission to open this block.");
                    ui.weak("Ask someone who can edit it to share it with you.");
                }
            });
        });
    }

    fn show_breadcrumbs(
        &mut self,
        ui: &mut egui::Ui,
        active: Uuid,
        current_name: &BlockLabel,
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
                let parent_label =
                    BlockLabel::for_reference(&self.registry, &parent).widget_text(ui.style());
                if ui
                    .button(parent_label)
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
        ui.label(current_name.widget_text(ui.style()));
        navigate
    }

    /// Drops the artifact dialogs a block may have open, so a block that is
    /// closed or has stopped being an artifact leaves nothing behind.
    fn forget_dynamic_artifact_dialogs(&mut self, id: Uuid) {
        self.dynamic_artifact_settings.remove(&id);
        if self.dynamic_artifact_settings_open == Some(id) {
            self.dynamic_artifact_settings_open = None;
        }
        if self.dynamic_artifact_unlink == Some(id) {
            self.dynamic_artifact_unlink = None;
        }
    }

    /// The bar above an artifact editor: where the block came from, what the
    /// generator is currently set to produce, and how to change, rerun or
    /// unlink it.
    fn show_dynamic_artifact_bar(
        &mut self,
        ui: &mut egui::Ui,
        id: Uuid,
        block_type: Uuid,
    ) -> Option<BlockTabHistoryItem> {
        let Some(descriptor) = self.client.dynamic_artifact(id) else {
            // A block that has just been unlinked keeps its tab open.
            self.forget_dynamic_artifact_dialogs(id);
            return None;
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

        let support = self.registry.dynamic_artifact(descriptor.source_type);
        let running = self.dynamic_artifact_regenerations.contains_key(&id);
        // The artifact is read-only in its editor, but rebuilding it is an edit
        // like any other, so it needs edit access to the block itself.
        let can_regenerate = self.client.block_access(id).can_edit();
        // The draft is held outside the UI so the bar only reads from `self`.
        let mut draft = self.dynamic_artifact_settings.remove(&id);
        let mut navigate = None;
        let mut regenerate = false;
        let mut open_settings = false;
        let mut open_unlink = false;
        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(egui::Margin::symmetric(8, 5))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(format!(
                        "{} Dynamic artifact",
                        ICON_DYNAMIC_ARTIFACT.codepoint
                    ));
                    match &support {
                        Ok(support) => {
                            navigate = self.show_dynamic_artifact_source(ui, &descriptor, *support);
                            ui.separator();
                            ui.weak((support.summary)(&descriptor.data));
                            open_settings = ui
                                .add_enabled(can_regenerate, egui::Button::new(ICON_SETTINGS))
                                .on_hover_text("Settings")
                                .on_disabled_hover_text(NO_EDIT_ACCESS)
                                .clicked();
                        }
                        Err(error) => {
                            ui.colored_label(ui.visuals().error_fg_color, error);
                        }
                    }
                    if running {
                        ui.spinner();
                    }
                    regenerate = ui
                        .add_enabled(
                            can_regenerate && !running && support.is_ok(),
                            egui::Button::new(ICON_REFRESH),
                        )
                        .on_hover_text("Regenerate")
                        .on_disabled_hover_text("You cannot change this block")
                        .clicked();
                    open_unlink = ui
                        .add_enabled(can_regenerate && !running, egui::Button::new(ICON_LINK_OFF))
                        .on_hover_text("Unlink from the source block")
                        .on_disabled_hover_text("You cannot change this block")
                        .clicked();
                });
                if let Some(error) = self.dynamic_artifact_errors.get(&id) {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });
        ui.separator();
        if open_settings {
            self.dynamic_artifact_settings_open = Some(id);
        }
        if open_unlink {
            self.dynamic_artifact_unlink = Some(id);
        }
        let mut apply = None;
        if self.dynamic_artifact_settings_open == Some(id) {
            match &support {
                Ok(support) => {
                    match self.show_dynamic_artifact_settings(
                        ui.ctx(),
                        &descriptor,
                        *support,
                        &mut draft,
                    ) {
                        ModalOutcome::Open => {}
                        ModalOutcome::Accepted(data) => {
                            apply = Some(data);
                            self.dynamic_artifact_settings_open = None;
                        }
                        ModalOutcome::Dismissed => {
                            draft = None;
                            self.dynamic_artifact_settings_open = None;
                        }
                    }
                }
                Err(_) => self.dynamic_artifact_settings_open = None,
            }
        }
        if self.dynamic_artifact_unlink == Some(id) {
            match show_dynamic_artifact_unlink(ui.ctx()) {
                ModalOutcome::Open => {}
                ModalOutcome::Accepted(()) => {
                    self.client.clear_dynamic_artifact(id);
                    self.dynamic_artifact_regenerations.remove(&id);
                    self.dynamic_artifact_errors.remove(&id);
                    self.forget_dynamic_artifact_dialogs(id);
                    return navigate;
                }
                ModalOutcome::Dismissed => self.dynamic_artifact_unlink = None,
            }
        }
        if let Some(data) = apply {
            self.client.set_dynamic_artifact(
                id,
                DynamicArtifactDescriptor {
                    source_type: descriptor.source_type,
                    data: data.clone(),
                },
            );
            self.regenerate_dynamic_artifact(id, block_type, descriptor.source_type, &data);
            draft = None;
        } else if regenerate {
            self.regenerate_dynamic_artifact(
                id,
                block_type,
                descriptor.source_type,
                &descriptor.data,
            );
        }
        if let Some(draft) = draft {
            self.dynamic_artifact_settings.insert(id, draft);
        }
        if self.dynamic_artifact_regenerations.contains_key(&id) {
            ui.ctx().request_repaint();
        }
        navigate
    }

    /// The bar above the editor of a block that is referenced from more than
    /// one place, warning that editing it here affects every place it
    /// appears.
    fn show_shared_block_bar(
        &mut self,
        ui: &mut egui::Ui,
        tab_id: Uuid,
        active: Uuid,
        relationships: Option<&block_client::BlockRelationships>,
    ) -> Option<BlockTabHistoryItem> {
        let relationships = relationships?;
        if relationships.backrefs.len() <= 1 {
            return None;
        }
        let count = relationships.backrefs.len();
        let container = self.opened_via.get(&active).copied();
        let via_reference =
            container.is_some_and(|container| relationships.parent != BlockParent::Uuid(container));
        let parent_id = match relationships.parent {
            BlockParent::Uuid(parent) => Some(parent),
            BlockParent::Root | BlockParent::Orphaned => None,
        };
        let parent_block_type = parent_id.and_then(|parent_id| {
            self.parents
                .get(&active)
                .map(ReferenceList::read)
                .and_then(|parents| {
                    parents
                        .last()
                        .filter(|parent| parent.id == parent_id)
                        .map(|parent| parent.block_type)
                })
                .or_else(|| self.block_types.get(&parent_id).copied())
        });
        let backrefs = self
            .backrefs
            .get(&active)
            .map(|list| (list.is_loaded(), list.read()));
        let container_block_type =
            container.and_then(|container| self.block_types.get(&container).copied());
        let copy_disabled_hover = match (container, container_block_type) {
            (Some(_), Some(block_type)) if !self.registry.can_replace_child(block_type) => {
                Some("This container doesn't support replacing a reference")
            }
            (Some(container), Some(_)) if !self.client.block_access(container).can_edit() => {
                Some("You don't have permission to edit this container")
            }
            (Some(_), Some(_)) => None,
            _ => Some("Loading…"),
        };
        let copy_enabled = copy_disabled_hover.is_none();
        let mut navigate = None;
        let mut context_action = None;
        let mut go_to_original = false;
        let mut make_copy = false;
        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(egui::Margin::symmetric(8, 5))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(format!("{} Shared block", ICON_LINK.codepoint));
                    if via_reference && parent_id.is_some() {
                        let others = count - 1;
                        ui.weak(format!(
                            "This block also appears in {others} other place{}. Editing it here changes it everywhere it appears.",
                            if others == 1 { "" } else { "s" }
                        ));
                        go_to_original = ui
                            .add_enabled(
                                parent_block_type.is_some(),
                                egui::Button::new("Go to original"),
                            )
                            .on_disabled_hover_text("Loading…")
                            .clicked();
                        let copy_button = ui.add_enabled(
                            copy_enabled,
                            egui::Button::new(format!(
                                "{} Make a copy",
                                ICON_CONTENT_COPY.codepoint
                            )),
                        );
                        make_copy = if let Some(hover) = copy_disabled_hover {
                            copy_button.on_disabled_hover_text(hover).clicked()
                        } else {
                            copy_button.clicked()
                        };
                    } else {
                        ui.weak(format!(
                            "This block appears in {count} places. Editing it here changes it everywhere."
                        ));
                        ui.menu_button("Show references", |ui| {
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
                        });
                    }
                });
            });
        ui.separator();
        if let Some((reference, source, is_reference, action)) = context_action {
            match action {
                BlockContextMenuAction::Picker => {
                    self.block_picker_target = Some(BlockPickerTarget::Block(reference.id));
                }
                BlockContextMenuAction::SetParent(parent) => {
                    self.set_block_parent(reference.id, parent);
                }
                BlockContextMenuAction::Rename => {
                    let name = BlockLabel::for_reference(&self.registry, &reference).name;
                    self.rename = Some(RenameState {
                        id: reference.id,
                        name,
                    });
                }
                BlockContextMenuAction::Share => {
                    let label = BlockLabel::for_reference(&self.registry, &reference);
                    self.share.open(&self.client, reference.id, label);
                }
                BlockContextMenuAction::Delete => {
                    self.queue_delete(reference, source, is_reference);
                }
            }
        }
        if go_to_original {
            if let (Some(parent_id), Some(block_type)) = (parent_id, parent_block_type) {
                self.opened_via.remove(&parent_id);
                navigate = Some((parent_id, block_type));
            }
        }
        if make_copy {
            if let Some(container) = container {
                self.queue_copy(active, container, tab_id);
            }
        }
        navigate.map(|(id, block_type)| BlockTabHistoryItem { id, block_type })
    }

    /// A link to the block the artifact was generated from.
    fn show_dynamic_artifact_source(
        &self,
        ui: &mut egui::Ui,
        descriptor: &DynamicArtifactDescriptor,
        support: DynamicArtifactSupport,
    ) -> Option<BlockTabHistoryItem> {
        let source = match (support.source)(&descriptor.data) {
            Ok(source) => source,
            Err(error) => {
                ui.colored_label(ui.visuals().error_fg_color, error);
                return None;
            }
        };
        ui.weak("Generated from");
        let label = self.client.cached_block(source).map_or_else(
            || BlockLabel {
                block_type: descriptor.source_type,
                icon: self.registry.icon(descriptor.source_type),
                name: self
                    .registry
                    .display_name(descriptor.source_type)
                    .unwrap_or("source block")
                    .to_owned(),
                automatic: true,
            },
            |block| BlockLabel::for_cached(&self.registry, &block),
        );
        ui.button(label.widget_text(ui.style()))
            .on_hover_text(format!("Open the source block\n{source}"))
            .clicked()
            .then_some(BlockTabHistoryItem {
                id: source,
                block_type: descriptor.source_type,
            })
    }

    /// The settings modal. Edits go to `draft` until they are applied, and
    /// dismissing the modal throws them away. A modal rather than a menu
    /// because a menu closes as soon as something inside it is clicked.
    fn show_dynamic_artifact_settings(
        &self,
        ctx: &egui::Context,
        descriptor: &DynamicArtifactDescriptor,
        support: DynamicArtifactSupport,
        draft: &mut Option<Vec<u8>>,
    ) -> ModalOutcome<Vec<u8>> {
        let mut outcome = ModalOutcome::Open;
        let response =
            egui::Modal::new(egui::Id::new("dynamic-artifact-settings")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.heading("Dynamic artifact settings");
                ui.add_space(12.0);
                let data = draft.get_or_insert_with(|| descriptor.data.clone());
                (support.settings_ui)(ui, data);
                ui.add_space(12.0);
                ui.weak((support.summary)(data));
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(*data != descriptor.data, egui::Button::new("Apply"))
                        .on_disabled_hover_text("The settings are unchanged")
                        .clicked()
                    {
                        outcome = ModalOutcome::Accepted(data.clone());
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = ModalOutcome::Dismissed;
                    }
                });
            });
        if matches!(outcome, ModalOutcome::Open) && response.should_close() {
            outcome = ModalOutcome::Dismissed;
        }
        outcome
    }

    fn regenerate_dynamic_artifact(
        &mut self,
        id: Uuid,
        block_type: Uuid,
        source_type: Uuid,
        data: &[u8],
    ) {
        self.dynamic_artifact_errors.remove(&id);
        match self.registry.regenerate_dynamic_artifact(
            source_type,
            &self.client,
            id,
            block_type,
            data,
        ) {
            Ok(regeneration) => {
                self.dynamic_artifact_regenerations.insert(id, regeneration);
            }
            Err(error) => {
                self.dynamic_artifact_errors.insert(id, error);
            }
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
            active_blocks: HashSet::new(),
        };
        DockArea::new(&mut dock_state).show_inside(ui, &mut viewer);
        for editor in viewer.app.editors.values_mut() {
            editor.finish_frame();
        }
        let active_blocks = std::mem::take(&mut viewer.active_blocks);
        viewer.app.update_active_presence(active_blocks);
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

    /// Reconciles [`UserActive`] presence with which blocks were actually on
    /// screen this frame: clears it for blocks that left, and posts it (with
    /// a color no one else visible on that block is already using) for
    /// blocks that newly appeared.
    fn update_active_presence(&mut self, visible: HashSet<Uuid>) {
        for id in self.active_presence.difference(&visible) {
            self.client.clear_presence::<UserActive>(*id);
            if let Some(editor) = self.editors.get_mut(id) {
                editor.sync_cursor_presence(&self.client, false);
            }
        }
        for id in visible.difference(&self.active_presence) {
            let used = self
                .client
                .presence::<UserActive>(*id)
                .into_iter()
                .map(|(_, user)| user.color);
            let color = pick_free_color(used);
            self.client.post_presence(*id, &UserActive { color });
        }
        for id in &visible {
            if let Some(editor) = self.editors.get_mut(id) {
                editor.sync_cursor_presence(&self.client, true);
            }
        }
        self.active_presence = visible;
    }

    fn show_statusbar(&mut self, ui: &mut egui::Ui, active: Uuid) -> Option<BlockTabHistoryItem> {
        let editor = self.editors.get(&active)?;
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

        let active_users = self.client.presence::<UserActive>(active);

        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Type: {type_name}"));
            if !active_users.is_empty() {
                ui.separator();
                ui.label("Also viewing:");
                for (_, user) in &active_users {
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 2.0, presence_color_rgb(user.color));
                    response.on_hover_text("Someone else is viewing this document");
                }
            }
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
                BlockContextMenuAction::SetParent(parent) => {
                    self.set_block_parent(reference.id, parent);
                }
                BlockContextMenuAction::Rename => {
                    let name = BlockLabel::for_reference(&self.registry, &reference).name;
                    self.rename = Some(RenameState {
                        id: reference.id,
                        name,
                    });
                }
                BlockContextMenuAction::Share => {
                    let label = BlockLabel::for_reference(&self.registry, &reference);
                    self.share.open(&self.client, reference.id, label);
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
            let label =
                BlockLabel::for_reference(&self.registry, reference).widget_text(ui.style());
            let response = ui.button(label).on_hover_text(reference.id.to_string());
            if response.clicked() {
                *navigate = Some((reference.id, reference.block_type));
                ui.close();
            }
            let can_edit = self.can_edit_block(reference.id);
            let permissions = BlockMenuPermissions {
                add: self.registry.can_add_child(reference.block_type) && can_edit,
                edit: can_edit,
                delete: source != SidebarDragSource::Orphaned
                    && self.can_move_out_of(source, reference.id, is_reference),
            };
            response.context_menu(|ui| {
                if let Some(action) = block_context_menu(
                    ui,
                    &self.registry,
                    &mut self.block_picker,
                    &self.client,
                    &mut self.parent_candidates,
                    reference.id,
                    reference.parent,
                    [reference.id],
                    permissions,
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
    /// Blocks whose tab is on screen this frame, collected as tabs are drawn
    /// so [`BlockApp::update_active_presence`] can tell who newly appeared
    /// and who left once the dock has finished drawing.
    active_blocks: HashSet<Uuid>,
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
                .map_or_else(
                    || egui::RichText::new(tab.current().id.to_string()),
                    |editor| BlockLabel::for_handle(&self.app.registry, editor.block()).rich_text(),
                )
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
                self.active_blocks.insert(current.id);
                let mut status_navigation = None;
                egui::Panel::bottom(egui::Id::new(("block-statusbar", tab.id)))
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        status_navigation = self.app.show_statusbar(ui, current.id)
                    });
                if let Some(item) = status_navigation {
                    self.navigations.push((tab.id, TabNavigation::Open(item)));
                }
                let (action, navigation) = self.app.show_content(
                    ui,
                    tab.id,
                    current.id,
                    tab.can_go_back(),
                    tab.can_go_forward(),
                );
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
    SetParent(BlockParent),
    Rename,
    Share,
    Delete,
}

/// What the account may do with the block a menu was opened on. Entries it is
/// not allowed are shown disabled rather than left out, so the menu reads the
/// same however much access the block was shared with.
struct BlockMenuPermissions {
    add: bool,
    edit: bool,
    delete: bool,
}

fn block_context_menu(
    ui: &mut egui::Ui,
    registry: &EditorRegistry,
    picker: &mut BlockPicker,
    client: &BlockClient,
    parent_candidates: &mut HashMap<Uuid, ReferenceList>,
    subject: Uuid,
    current_parent: BlockParent,
    excluded: impl IntoIterator<Item = Uuid>,
    permissions: BlockMenuPermissions,
) -> Option<BlockContextMenuAction> {
    let mut action = None;
    ui.add_enabled_ui(permissions.add, |ui| {
        if ui.button("Add").clicked() {
            picker.open(excluded);
            action = Some(BlockContextMenuAction::Picker);
            ui.close();
        }
    })
    .response
    .on_disabled_hover_text(NO_EDIT_ACCESS);
    ui.add_enabled_ui(permissions.edit, |ui| {
        ui.menu_button("Set parent", |ui| {
            if ui
                .add_enabled(
                    current_parent != BlockParent::Root,
                    egui::Button::new("Root"),
                )
                .clicked()
            {
                action = Some(BlockContextMenuAction::SetParent(BlockParent::Root));
                ui.close();
            }
            if ui
                .add_enabled(
                    current_parent != BlockParent::Orphaned,
                    egui::Button::new("Orphaned"),
                )
                .clicked()
            {
                action = Some(BlockContextMenuAction::SetParent(BlockParent::Orphaned));
                ui.close();
            }
            ui.separator();
            let backrefs = parent_candidates
                .entry(subject)
                .or_insert_with(|| client.watch_references(BlockReferenceList::Backrefs(subject)));
            let listed = backrefs.read();
            if listed.is_empty() {
                ui.weak(if backrefs.is_loaded() {
                    "No backrefs"
                } else {
                    "Loading…"
                });
            }
            for backref in listed {
                let is_current = current_parent == BlockParent::Uuid(backref.id);
                let label = BlockLabel::for_reference(registry, &backref).widget_text(ui.style());
                if ui
                    .add_enabled(!is_current, egui::Button::new(label))
                    .clicked()
                {
                    action = Some(BlockContextMenuAction::SetParent(BlockParent::Uuid(
                        backref.id,
                    )));
                    ui.close();
                }
            }
        });
    })
    .response
    .on_disabled_hover_text(NO_EDIT_ACCESS);
    if ui
        .add_enabled(permissions.edit, egui::Button::new("Rename"))
        .on_disabled_hover_text(NO_EDIT_ACCESS)
        .clicked()
    {
        action = Some(BlockContextMenuAction::Rename);
        ui.close();
    }
    if ui
        .add_enabled(
            permissions.edit,
            egui::Button::new(format!("{} Share", ICON_SHARE.codepoint)),
        )
        .on_disabled_hover_text("Only accounts that can edit a block may share it")
        .clicked()
    {
        action = Some(BlockContextMenuAction::Share);
        ui.close();
    }
    let delete_text = egui::RichText::new("Delete");
    let delete_text = if permissions.delete {
        delete_text.color(ui.visuals().error_fg_color)
    } else {
        delete_text
    };
    if ui
        .add_enabled(permissions.delete, egui::Button::new(delete_text))
        .clicked()
    {
        action = Some(BlockContextMenuAction::Delete);
        ui.close();
    }
    action
}

/// What a modal did with the frame it was drawn in: nothing yet, went ahead
/// with what it was opened for, or was closed without doing it.
enum ModalOutcome<T> {
    Open,
    Accepted(T),
    Dismissed,
}

/// Confirms unlinking an artifact from its source. The generated value stays
/// where it is, so the only thing lost is the link and its settings.
fn show_dynamic_artifact_unlink(ctx: &egui::Context) -> ModalOutcome<()> {
    let mut outcome = ModalOutcome::Open;
    let response = egui::Modal::new(egui::Id::new("dynamic-artifact-unlink")).show(ctx, |ui| {
        ui.set_width(360.0);
        ui.heading("Unlink from the source block?");
        ui.add_space(12.0);
        ui.label("This block keeps what was generated for it, but stops being rebuilt from its source and becomes editable.");
        ui.label("The link and its settings cannot be restored.");
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Unlink").clicked() {
                outcome = ModalOutcome::Accepted(());
            }
            if ui.button("Cancel").clicked() {
                outcome = ModalOutcome::Dismissed;
            }
        });
    });
    if matches!(outcome, ModalOutcome::Open) && response.should_close() {
        outcome = ModalOutcome::Dismissed;
    }
    outcome
}

/// What a tab's mode dropdown is currently showing: the block simulated at
/// some level of access, or its raw serialized data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabMode {
    Access(BlockAccess),
    Debug,
}

/// The mode a tab is showing its block in. Access modes above what the
/// account is allowed cannot be picked; the ones below it show what someone
/// with less access would see. Debug is only offered once the block can be
/// viewed at all. Returns the mode to show the block in from now on.
fn show_access_mode(
    ui: &mut egui::Ui,
    active: Uuid,
    access: BlockAccess,
    ceiling: BlockAccess,
    generated: bool,
    debug: bool,
) -> TabMode {
    let current = if debug {
        TabMode::Debug
    } else {
        TabMode::Access(access)
    };
    let mut chosen = current;
    egui::ComboBox::from_id_salt(("editor-access-mode", active))
        .selected_text(tab_mode_label(current))
        .show_ui(ui, |ui| {
            for mode in [BlockAccess::Edit, BlockAccess::View, BlockAccess::KnowExists] {
                let label = access_mode_label(mode);
                ui.add_enabled_ui(mode <= ceiling, |ui| {
                    if ui.selectable_label(current == TabMode::Access(mode), label).clicked() {
                        chosen = TabMode::Access(mode);
                    }
                })
                .response
                .on_disabled_hover_text(if generated {
                    "Generated blocks are replaced whenever they are rebuilt, so they cannot be edited here"
                } else {
                    "You do not have that much access to this block"
                });
            }
            ui.separator();
            ui.add_enabled_ui(ceiling.can_view(), |ui| {
                if ui
                    .selectable_label(current == TabMode::Debug, debug_mode_label())
                    .clicked()
                {
                    chosen = TabMode::Debug;
                }
            })
            .response
            .on_disabled_hover_text("You do not have that much access to this block");
        })
        .response
        .on_hover_text(
            "Show this block as an account with this much access would see it, or inspect its raw data",
        );
    chosen
}

/// An access mode named for a menu, where every mode is spelled out.
fn access_mode_label(access: BlockAccess) -> String {
    let icon = access_mode_icon(access).unwrap_or(ICON_EDIT.codepoint);
    format!("{icon} {}", tab_mode_wording(access))
}

/// The wording this dropdown uses for each access level. Kept separate from
/// `BlockAccess::label`, whose wording fits the sharing dialog instead.
fn tab_mode_wording(access: BlockAccess) -> &'static str {
    match access {
        BlockAccess::Edit => "Editing",
        BlockAccess::View => "Viewing",
        BlockAccess::KnowExists | BlockAccess::None => "No access",
    }
}

fn debug_mode_label() -> String {
    format!("{} Debug", ICON_DATA_OBJECT.codepoint)
}

fn tab_mode_label(mode: TabMode) -> String {
    match mode {
        TabMode::Access(access) => access_mode_label(access),
        TabMode::Debug => debug_mode_label(),
    }
}

/// How a block's access is marked where it is listed. Editing is what every
/// block allows until it is shared more narrowly, so it goes unmarked.
fn access_mode_icon(access: BlockAccess) -> Option<&'static str> {
    match access {
        BlockAccess::Edit => None,
        BlockAccess::View => Some(ICON_VISIBILITY.codepoint),
        BlockAccess::KnowExists | BlockAccess::None => Some(ICON_LOCK.codepoint),
    }
}

/// The concrete color an active-user indicator is painted in.
fn presence_color_rgb(color: PresenceColor) -> egui::Color32 {
    match color {
        PresenceColor::Red => egui::Color32::from_rgb(224, 82, 82),
        PresenceColor::Orange => egui::Color32::from_rgb(230, 140, 50),
        PresenceColor::Yellow => egui::Color32::from_rgb(214, 179, 41),
        PresenceColor::Green => egui::Color32::from_rgb(84, 171, 90),
        PresenceColor::Teal => egui::Color32::from_rgb(46, 173, 168),
        PresenceColor::Blue => egui::Color32::from_rgb(74, 134, 227),
        PresenceColor::Purple => egui::Color32::from_rgb(150, 100, 214),
        PresenceColor::Pink => egui::Color32::from_rgb(224, 104, 168),
    }
}

enum AccountAction {
    Open(Account),
    ChooseWorkspace(Account),
    LogOut(Account),
}

/// Lays out onboarding content in a scrollable, horizontally centred column.
fn onboarding_column<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let width = ONBOARDING_WIDTH.min(ui.available_width());
            let margin = ((ui.available_width() - width) / 2.0).max(0.0);
            ui.horizontal_top(|ui| {
                ui.add_space(margin);
                ui.allocate_ui_with_layout(
                    egui::vec2(width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(width);
                        add_contents(ui)
                    },
                )
                .inner
            })
            .inner
        })
        .inner
}

fn onboarding_card<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui)
        })
        .inner
}

fn account_name(ui: &mut egui::Ui, account: &Account) {
    ui.add(egui::Label::new(egui::RichText::new(account.name.as_str()).strong()).truncate());
}

fn account_details(ui: &mut egui::Ui, account: &Account) {
    ui.add(egui::Label::new(egui::RichText::new(account.email.as_str()).small()).truncate());
    let server = match &account.server {
        ServerLocation::Local => format!("{} Local server", ICON_COMPUTER.codepoint),
        ServerLocation::Remote(url) => format!("{} {url}", ICON_CLOUD.codepoint),
    };
    ui.add(egui::Label::new(egui::RichText::new(server).small().weak()).truncate());
}

fn show_account_card(ui: &mut egui::Ui, account: &Account) -> Option<AccountAction> {
    let mut log_out = false;
    let mut open = false;
    let mut choose_workspace = false;
    onboarding_card(ui, |ui| {
        egui::Sides::new().shrink_left().show(
            ui,
            |ui| account_name(ui, account),
            |ui| {
                ui.menu_button(ICON_MORE_HORIZ, |ui| {
                    if ui
                        .button(format!("{} Log out", ICON_LOGOUT.codepoint))
                        .clicked()
                    {
                        log_out = true;
                        ui.close();
                    }
                })
                .response
                .on_hover_text("Account options");
            },
        );
        account_details(ui, account);
        ui.add_space(8.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            choose_workspace = ui
                .add(egui::Button::new(ICON_KEYBOARD_ARROW_DOWN).selected(true))
                .on_hover_text("Open a different workspace")
                .clicked();
            open = ui
                .add(egui::Button::new("Open").selected(true))
                .on_hover_text(match account.last_workspace_id {
                    Some(_) => "Open the last workspace used",
                    None => "Choose a workspace",
                })
                .clicked();
        });
    });
    if log_out {
        Some(AccountAction::LogOut(account.clone()))
    } else if choose_workspace {
        Some(AccountAction::ChooseWorkspace(account.clone()))
    } else if open {
        Some(AccountAction::Open(account.clone()))
    } else {
        None
    }
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
        if self.scheduled_workspace_list {
            self.scheduled_workspace_list = false;
            let mut account = self.account.clone();
            account.last_workspace_id = None;
            let _ = self.app_state.set_last_workspace(&account, None);
            self.switch_account(ui.ctx(), account);
        }
        if let Some(account) = self.scheduled_account_switch.take() {
            self.switch_account(ui.ctx(), account);
        }
        if self.workspace.is_none() {
            self.show_workspace_onboarding(ui);
            performance::end_frame();
            ui.ctx().request_repaint_after(Duration::from_millis(100));
            return;
        }
        self.poll_workspace_request();
        self.intercept_close(ui.ctx());
        self.process_pending_transfers();
        self.process_pending_copies();
        self.show_block_picker(ui.ctx());
        self.show_rename(ui);
        self.share.show(ui.ctx(), &self.client);
        self.show_client_debug(ui.ctx());
        self.show_network_debug(ui.ctx());
        #[cfg(not(any(
            target_os = "android",
            target_os = "windows",
            target_os = "macos",
            target_arch = "wasm32"
        )))]
        debug::terminal::show(ui.ctx());
        self.show_invite(ui.ctx());
        self.show_about(ui.ctx());

        self.show_dock(ui, frame);
        self.show_discard_confirmation(ui.ctx());
        performance::show(ui.ctx());
        performance::end_frame();
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
}
