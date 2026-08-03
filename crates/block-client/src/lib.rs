use std::{
    any::Any,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    fmt,
    ops::Deref,
    ops::Range,
    process,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, OnceLock, Weak,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use block::{
    Account, Block, BlockAccess, BlockAccessEntry, BlockHistory, BlockHistoryTransaction,
    BlockOperation, BlockParent, BlockReference, BlockReferenceList, BlockUpdate, ClientMessage,
    CommandKind, ErrorCode, HistoryDirection, ManagementClientMessage, ManagementErrorCode,
    ManagementServerMessage, OperationRecord, ReferenceDelta, ServerMessage, Workspace,
    WorkspaceInvitation, WorkspaceRole,
};
use futures_channel::mpsc;
use futures_util::{FutureExt, StreamExt};
use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use serde::{Deserialize, Serialize};
use tokio::sync::{oneshot, watch};
use uuid::Uuid;

pub mod blocks;
mod transport;

use transport::{Socket, SocketMessage};

/// Sends commands to the worker. Sending never blocks, so the UI thread can
/// queue work from inside a frame, and the same channel drives the worker
/// whether it runs on a thread or on the browser's event loop.
#[derive(Clone)]
struct CommandSender(mpsc::UnboundedSender<WorkerCommand>);

impl CommandSender {
    fn send(&self, command: WorkerCommand) -> Result<(), mpsc::TrySendError<WorkerCommand>> {
        self.0.unbounded_send(command)
    }
}

/// Set when the [`BlockClient`] is dropped, so the worker can tell a connection
/// it still needs from one nobody is waiting on any more. The worker outlives
/// its client by however long it takes to notice the command channel closed,
/// and whatever shut the client down usually takes the connection with it, so a
/// socket that fails inside that window is a shutdown rather than a fault.
#[derive(Clone, Default)]
struct Shutdown(Arc<AtomicBool>);

impl Shutdown {
    fn request(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    fn requested(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Block connections identify themselves in the websocket URL's query string.
/// Browsers cannot set request headers on a websocket handshake, so the query
/// string is the only place a web client can put this.
const ACCOUNT_PARAMETER: &str = "account";
const WORKSPACE_PARAMETER: &str = "workspace";
/// Where the server answers management commands, relative to the server URL.
const MANAGEMENT_PATH: &str = "/management";
const BLOCK_URL_BASE: &str = "https://blocks.pfg.pw/";
const UUID_TEXT_BYTES: usize = 36;
pub const BLOCK_URL_BYTES: usize = BLOCK_URL_BASE.len() + UUID_TEXT_BYTES * 2 + 1;
const MAX_HISTORY_ACTIONS: usize = 100;
const MAX_HISTORY_BYTES: usize = 64 * 1024 * 1024;

/// Account and workspace management, which the server answers over HTTP. Block
/// traffic uses the websocket [`BlockClient`] opens instead.
#[derive(Clone, Debug)]
pub struct ManagementClient {
    url: String,
}

#[derive(Debug)]
pub enum ManagementClientError {
    InvalidUrl(String),
    Transport(String),
    InvalidResponse(String),
    Server {
        code: ManagementErrorCode,
        message: String,
    },
}

impl fmt::Display for ManagementClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl(message) | Self::InvalidResponse(message) => {
                formatter.write_str(message)
            }
            Self::Transport(message) => write!(formatter, "management request failed: {message}"),
            Self::Server { message, .. } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ManagementClientError {}

impl ManagementClient {
    pub fn new(url: impl Into<String>) -> Result<Self, ManagementClientError> {
        let url = url.into().trim().trim_end_matches('/').to_owned();
        let host = ["http://", "https://"]
            .iter()
            .find_map(|scheme| url.strip_prefix(scheme));
        if host.is_none_or(str::is_empty) {
            return Err(ManagementClientError::InvalidUrl(
                "server URL must start with http:// or https://".into(),
            ));
        }
        Ok(Self { url })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn register(
        &self,
        email: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Account, ManagementClientError> {
        let response = self
            .request(ManagementClientMessage::Register {
                request_id: Uuid::new_v4(),
                email: email.into(),
                display_name: display_name.into(),
            })
            .await?;
        let ManagementServerMessage::Account { account, .. } = response else {
            return Err(unexpected_management_response(response));
        };
        Ok(account)
    }

    pub async fn login(&self, email: impl Into<String>) -> Result<Account, ManagementClientError> {
        let response = self
            .request(ManagementClientMessage::Login {
                request_id: Uuid::new_v4(),
                email: email.into(),
            })
            .await?;
        let ManagementServerMessage::Account { account, .. } = response else {
            return Err(unexpected_management_response(response));
        };
        Ok(account)
    }

    pub async fn list_workspaces(
        &self,
        account_id: Uuid,
    ) -> Result<Vec<Workspace>, ManagementClientError> {
        let response = self
            .request(ManagementClientMessage::ListWorkspaces {
                request_id: Uuid::new_v4(),
                account_id,
            })
            .await?;
        let ManagementServerMessage::Workspaces { workspaces, .. } = response else {
            return Err(unexpected_management_response(response));
        };
        Ok(workspaces)
    }

    pub async fn create_workspace(
        &self,
        account_id: Uuid,
        name: impl Into<String>,
    ) -> Result<Workspace, ManagementClientError> {
        let response = self
            .request(ManagementClientMessage::CreateWorkspace {
                request_id: Uuid::new_v4(),
                account_id,
                name: name.into(),
            })
            .await?;
        let ManagementServerMessage::Workspace { workspace, .. } = response else {
            return Err(unexpected_management_response(response));
        };
        Ok(workspace)
    }

    pub async fn list_invitations(
        &self,
        account_id: Uuid,
    ) -> Result<Vec<WorkspaceInvitation>, ManagementClientError> {
        let response = self
            .request(ManagementClientMessage::ListInvitations {
                request_id: Uuid::new_v4(),
                account_id,
            })
            .await?;
        let ManagementServerMessage::Invitations { invitations, .. } = response else {
            return Err(unexpected_management_response(response));
        };
        Ok(invitations)
    }

    pub async fn invite(
        &self,
        account_id: Uuid,
        workspace_id: Uuid,
        email: impl Into<String>,
        role: WorkspaceRole,
    ) -> Result<WorkspaceInvitation, ManagementClientError> {
        let response = self
            .request(ManagementClientMessage::Invite {
                request_id: Uuid::new_v4(),
                account_id,
                workspace_id,
                email: email.into(),
                role,
            })
            .await?;
        let ManagementServerMessage::Invitation { invitation, .. } = response else {
            return Err(unexpected_management_response(response));
        };
        Ok(invitation)
    }

    pub async fn respond_invitation(
        &self,
        account_id: Uuid,
        invitation_id: Uuid,
        accept: bool,
    ) -> Result<(), ManagementClientError> {
        let response = self
            .request(ManagementClientMessage::RespondInvitation {
                request_id: Uuid::new_v4(),
                account_id,
                invitation_id,
                accept,
            })
            .await?;
        let ManagementServerMessage::Ok { .. } = response else {
            return Err(unexpected_management_response(response));
        };
        Ok(())
    }

    async fn request(
        &self,
        request: ManagementClientMessage,
    ) -> Result<ManagementServerMessage, ManagementClientError> {
        let request_id = request.request_id();
        let body = serde_json::to_vec(&request)
            .map_err(|error| ManagementClientError::InvalidResponse(error.to_string()))?;
        let url = format!("{}{MANAGEMENT_PATH}", self.url);
        let (status, text) = transport::post_json(url, body)
            .await
            .map_err(ManagementClientError::Transport)?;
        let response: ManagementServerMessage = serde_json::from_str(&text).map_err(|error| {
            ManagementClientError::InvalidResponse(format!(
                "management server returned an unreadable HTTP {status} response: {error}"
            ))
        })?;
        match response {
            ManagementServerMessage::Error { code, message, .. } => {
                Err(ManagementClientError::Server { code, message })
            }
            response if management_response_request_id(&response) == request_id => Ok(response),
            _ => Err(ManagementClientError::InvalidResponse(
                "management response request ID did not match".into(),
            )),
        }
    }
}

fn management_response_request_id(response: &ManagementServerMessage) -> Uuid {
    match response {
        ManagementServerMessage::Account { request_id, .. }
        | ManagementServerMessage::Workspace { request_id, .. }
        | ManagementServerMessage::Workspaces { request_id, .. }
        | ManagementServerMessage::Invitation { request_id, .. }
        | ManagementServerMessage::Invitations { request_id, .. }
        | ManagementServerMessage::Ok { request_id } => *request_id,
        ManagementServerMessage::Error { .. } => Uuid::nil(),
    }
}

fn unexpected_management_response(response: ManagementServerMessage) -> ManagementClientError {
    ManagementClientError::InvalidResponse(format!(
        "management server returned an unexpected response: {response:?}"
    ))
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DynamicArtifactDescriptor {
    pub source_type: Uuid,
    pub data: Vec<u8>,
}

#[derive(Deserialize, Serialize)]
struct StoredBlock<B> {
    value: B,
    dynamic_artifact: Option<DynamicArtifactDescriptor>,
}

#[derive(Clone, Deserialize, Serialize)]
enum StoredOperation<B, O> {
    Operate(O),
    Replace(B),
}

#[cfg(test)]
mod tests;

pub struct BlockClient {
    id: Uuid,
    account_id: Uuid,
    workspace_id: Uuid,
    commands: CommandSender,
    connected: Arc<OnceLock<()>>,
    shutdown: Shutdown,
    access: Arc<RwLock<()>>,
    debug: Arc<RwLock<NetworkDebugSnapshot>>,
    client_debug: Arc<RwLock<ClientDebugSnapshot>>,
    cached_blocks: Arc<RwLock<HashMap<Uuid, CachedBlock>>>,
    registered_blocks: Arc<RwLock<HashMap<Uuid, RegisteredBlock>>>,
    watched_reference_lists: Arc<RwLock<HashMap<BlockReferenceList, Weak<ReferenceListShared>>>>,
    denied_blocks: Arc<RwLock<HashSet<Uuid>>>,
}

/// A sharing request that is still in flight. The UI polls it each frame rather
/// than blocking, so it never has to wait on the server to draw.
pub struct BlockAccessRequest {
    id: Uuid,
    completed: oneshot::Receiver<Result<Vec<BlockAccessEntry>, String>>,
}

impl BlockAccessRequest {
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Returns the outcome once the server has answered, and `None` until then.
    pub fn poll(&mut self) -> Option<Result<Vec<BlockAccessEntry>, String>> {
        match self.completed.try_recv() {
            Ok(result) => Some(result),
            Err(oneshot::error::TryRecvError::Empty) => None,
            Err(oneshot::error::TryRecvError::Closed) => {
                Some(Err("the block client stopped before answering".into()))
            }
        }
    }
}

struct RegisteredBlock {
    block_type: Uuid,
    typed: Arc<dyn Any + Send + Sync>,
    erased: Arc<dyn ErasedBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedBlock {
    pub id: Uuid,
    pub block_type: Uuid,
    pub author: Uuid,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockUrl {
    pub range: Range<usize>,
    pub workspace_id: Uuid,
    pub id: Uuid,
}

pub fn block_url_prefix(workspace_id: Uuid) -> String {
    format!("{BLOCK_URL_BASE}{workspace_id}/")
}

pub fn block_url(workspace_id: Uuid, id: Uuid) -> String {
    format!("{}{id}", block_url_prefix(workspace_id))
}

pub fn parse_block_urls(bytes: &[u8]) -> Vec<BlockUrl> {
    let prefix = BLOCK_URL_BASE.as_bytes();
    let mut urls = Vec::new();
    let mut index = 0;
    while index + BLOCK_URL_BYTES <= bytes.len() {
        if !bytes[index..].starts_with(prefix) {
            index += 1;
            continue;
        }
        let workspace_start = index + prefix.len();
        let workspace_end = workspace_start + UUID_TEXT_BYTES;
        let Ok(workspace) = std::str::from_utf8(&bytes[workspace_start..workspace_end]) else {
            index += 1;
            continue;
        };
        let Ok(workspace_id) = Uuid::parse_str(workspace) else {
            index += 1;
            continue;
        };
        if bytes.get(workspace_end) != Some(&b'/') {
            index += 1;
            continue;
        }
        let id_start = workspace_end + 1;
        let end = id_start + UUID_TEXT_BYTES;
        let Ok(id) = std::str::from_utf8(&bytes[id_start..end])
            .ok()
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or(())
        else {
            index += 1;
            continue;
        };
        if bytes
            .get(end)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'/'))
        {
            index += 1;
            continue;
        }
        urls.push(BlockUrl {
            range: index..end,
            workspace_id,
            id,
        });
        index = end;
    }
    urls
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkDirection {
    Sent,
    Received,
}

#[derive(Clone, Debug)]
pub struct NetworkTrafficEntry {
    pub timestamp_ms: u128,
    pub direction: NetworkDirection,
    pub payload: String,
}

#[derive(Clone, Debug, Default)]
pub struct NetworkDebugSnapshot {
    pub sending_paused: bool,
    pub queued_messages: usize,
    pub changes_saved: bool,
    pub traffic: Vec<NetworkTrafficEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientDebugSnapshot {
    pub client_id: Uuid,
    pub account_id: Uuid,
    pub workspace_id: Uuid,
    pub connected: bool,
    pub sending_paused: bool,
    pub queued_messages: usize,
    pub steps_remaining: usize,
    pub changes_saved: bool,
    pub synchronization_waiters: usize,
    pub blocks: Vec<BlockDebugSnapshot>,
    pub reference_lists: Vec<ReferenceListDebugSnapshot>,
    pub cached_blocks: Vec<CachedBlock>,
    pub pending_requests: Vec<PendingRequestDebugSnapshot>,
    pub outbound_messages: Vec<ClientDebugEntry>,
    pub deferred_requests: Vec<ClientDebugEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockDebugSnapshot {
    pub id: Uuid,
    pub block_type: Uuid,
    pub name: String,
    pub crdt: bool,
    pub ready: bool,
    pub synchronized: bool,
    pub has_local_changes: bool,
    pub confirmed_seq: u64,
    pub acknowledged_seq: u64,
    pub pending_operations: usize,
    pub in_flight_operations: usize,
    pub buffered_operations: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceListDebugSnapshot {
    pub list: BlockReferenceList,
    pub loaded: bool,
    pub blocks: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRequestDebugSnapshot {
    pub request_id: Uuid,
    pub kind: String,
    pub details: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientDebugEntry {
    pub kind: String,
    pub details: String,
}

impl ClientDebugSnapshot {
    fn empty(client_id: Uuid, account_id: Uuid, workspace_id: Uuid) -> Self {
        Self {
            client_id,
            account_id,
            workspace_id,
            connected: false,
            sending_paused: false,
            queued_messages: 0,
            steps_remaining: 0,
            changes_saved: true,
            synchronization_waiters: 0,
            blocks: Vec::new(),
            reference_lists: Vec::new(),
            cached_blocks: Vec::new(),
            pending_requests: Vec::new(),
            outbound_messages: Vec::new(),
            deferred_requests: Vec::new(),
        }
    }
}

impl BlockClient {
    pub fn new(account_id: Uuid, workspace_id: Uuid) -> Self {
        let (commands, command_rx) = mpsc::unbounded();
        let commands = CommandSender(commands);
        let id = Uuid::new_v4();
        let connected = Arc::new(OnceLock::new());
        let shutdown = Shutdown::default();
        let access = Arc::new(RwLock::new(()));
        let debug = Arc::new(RwLock::new(NetworkDebugSnapshot {
            changes_saved: true,
            ..Default::default()
        }));
        let client_debug = Arc::new(RwLock::new(ClientDebugSnapshot::empty(
            id,
            account_id,
            workspace_id,
        )));
        let cached_blocks = Arc::new(RwLock::new(HashMap::new()));
        let registered_blocks = Arc::new(RwLock::new(HashMap::new()));
        let watched_reference_lists = Arc::new(RwLock::new(HashMap::new()));
        let denied_blocks = Arc::new(RwLock::new(HashSet::new()));
        let worker_access = Arc::clone(&access);
        let worker_debug = Arc::clone(&debug);
        let worker_client_debug = Arc::clone(&client_debug);
        let worker_cached_blocks = Arc::clone(&cached_blocks);
        let worker_denied_blocks = Arc::clone(&denied_blocks);
        transport::spawn_worker(worker_main(
            command_rx,
            shutdown.clone(),
            worker_access,
            worker_debug,
            worker_client_debug,
            worker_cached_blocks,
            worker_denied_blocks,
        ));
        Self {
            id,
            account_id,
            workspace_id,
            commands,
            connected,
            shutdown,
            access,
            debug,
            client_debug,
            cached_blocks,
            registered_blocks,
            watched_reference_lists,
            denied_blocks,
        }
    }

    /// Connects to the block websocket of the server at `url`, which is the same
    /// `http://` or `https://` URL [`ManagementClient`] is given.
    pub fn connect(&self, url: impl Into<String>) {
        if self.connected.set(()).is_err() {
            fatal("BlockClient::connect may only be called once");
        }
        self.send(WorkerCommand::Connect {
            url: url.into(),
            account_id: self.account_id,
            workspace_id: self.workspace_id,
        });
    }

    pub fn account_id(&self) -> Uuid {
        self.account_id
    }

    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }

    pub fn create_block<B: Block>(&self, initial: B) -> BlockHandle<B> {
        self.create_block_inner(initial, None)
    }

    pub fn create_dynamic_artifact<B: Block>(
        &self,
        initial: B,
        descriptor: DynamicArtifactDescriptor,
    ) -> BlockHandle<B> {
        self.create_block_inner(initial, Some(descriptor))
    }

    fn create_block_inner<B: Block>(
        &self,
        initial: B,
        dynamic_artifact: Option<DynamicArtifactDescriptor>,
    ) -> BlockHandle<B> {
        let id = Uuid::new_v4();
        let shared = Arc::new(BlockShared {
            value: RwLock::new(Some(initial.clone())),
        });
        let block = Arc::new(TypedBlock::<B>::created_by(
            id,
            self.account_id,
            self.workspace_id,
            Arc::clone(&shared),
            initial,
            dynamic_artifact,
        ));
        self.register_block(id, &block);
        self.send(WorkerCommand::AddBlock(block.clone()));
        self.block_handle(id, block)
    }

    pub fn get_block<B: Block>(&self, id: Uuid) -> BlockHandle<B> {
        if let Some(registered) = self.registered_blocks.read().get(&id) {
            if registered.block_type != B::TYPE_ID {
                fatal(format!(
                    "block {id} is registered as {}, not {}",
                    registered.block_type,
                    B::TYPE_ID
                ));
            }
            let typed = Arc::clone(&registered.typed);
            let block = Arc::downcast::<TypedBlock<B>>(typed).unwrap_or_else(|_| {
                fatal(format!("block {id} has an inconsistent registered type"))
            });
            return self.block_handle(id, block);
        }
        let shared = Arc::new(BlockShared {
            value: RwLock::new(None),
        });
        let block = Arc::new(TypedBlock::<B>::unresolved(
            id,
            self.workspace_id,
            Arc::clone(&shared),
        ));
        self.register_block(id, &block);
        self.send(WorkerCommand::AddBlock(block.clone()));
        self.block_handle(id, block)
    }

    pub fn dynamic_artifact(&self, id: Uuid) -> Option<DynamicArtifactDescriptor> {
        self.registered_blocks
            .read()
            .get(&id)
            .and_then(|registered| registered.erased.dynamic_artifact())
    }

    fn register_block<B: Block>(&self, id: Uuid, block: &Arc<TypedBlock<B>>) {
        let registered = RegisteredBlock {
            block_type: B::TYPE_ID,
            typed: block.clone(),
            erased: block.clone(),
        };
        if self
            .registered_blocks
            .write()
            .insert(id, registered)
            .is_some()
        {
            fatal(format!("block {id} is already registered"));
        }
    }

    fn block_handle<B: Block>(&self, id: Uuid, block: Arc<TypedBlock<B>>) -> BlockHandle<B> {
        BlockHandle {
            client_id: self.id,
            id,
            block,
            commands: self.commands.clone(),
            access: Arc::clone(&self.access),
        }
    }

    pub fn batch(&self, update: impl FnOnce(&mut BlockBatch<'_>)) {
        let mut batch = BlockBatch {
            client_id: self.id,
            commands: self.commands.clone(),
            _access: self.access.write(),
            ids: Vec::new(),
        };
        update(&mut batch);
    }

    pub async fn synchronized(&self) {
        let (completed, completion) = oneshot::channel();
        self.send(WorkerCommand::Synchronize(completed));
        completion
            .await
            .unwrap_or_else(|_| fatal("block client worker stopped before synchronizing"));
    }

    pub async fn list_references(&self, list: BlockReferenceList) -> Vec<BlockReference> {
        let (completed, completion) = oneshot::channel();
        self.send(WorkerCommand::ListReferences { list, completed });
        completion
            .await
            .unwrap_or_else(|_| fatal("block client worker stopped before listing references"))
    }

    pub fn watch_references(&self, list: BlockReferenceList) -> ReferenceList {
        let mut watched = self.watched_reference_lists.write();
        if let Some(shared) = watched.get(&list).and_then(Weak::upgrade) {
            shared.subscribers.fetch_add(1, Ordering::Relaxed);
            return ReferenceList {
                list,
                shared,
                commands: self.commands.clone(),
                watched_reference_lists: Arc::clone(&self.watched_reference_lists),
            };
        }
        let shared = Arc::new(ReferenceListShared {
            blocks: RwLock::new(Vec::new()),
            loaded: watch::channel(false).0,
            subscribers: AtomicUsize::new(1),
        });
        watched.insert(list, Arc::downgrade(&shared));
        self.send(WorkerCommand::WatchReferences {
            list,
            shared: Arc::clone(&shared),
        });
        ReferenceList {
            list,
            shared,
            commands: self.commands.clone(),
            watched_reference_lists: Arc::clone(&self.watched_reference_lists),
        }
    }

    pub fn watch_parents(&self, id: Uuid) -> ReferenceList {
        self.watch_references(BlockReferenceList::Parents(id))
    }

    pub fn set_block_name(&self, id: Uuid, name: impl Into<String>) {
        self.send(WorkerCommand::SetBlockName {
            id,
            name: name.into(),
        });
    }

    pub fn set_block_parent(&self, id: Uuid, parent: BlockParent) {
        self.send(WorkerCommand::SetBlockParent { id, parent });
    }

    /// Asks the server who can reach `id`, for the sharing UI. The caller polls
    /// the returned request rather than awaiting it.
    pub fn request_block_access(&self, id: Uuid) -> BlockAccessRequest {
        let (completed, receiver) = oneshot::channel();
        self.send(WorkerCommand::ListBlockAccess { id, completed });
        BlockAccessRequest {
            id,
            completed: receiver,
        }
    }

    /// Grants `account_id` the given access to `id`; [`BlockAccess::None`]
    /// withdraws whatever was granted before.
    pub fn set_block_access(&self, id: Uuid, account_id: Uuid, access: BlockAccess) {
        self.send(WorkerCommand::SetBlockAccess {
            id,
            account_id,
            access,
        });
    }

    /// Whether the server has refused this client access to `id`. Blocks in
    /// this state never load, so editors show an explanation instead.
    pub fn access_denied(&self, id: Uuid) -> bool {
        self.denied_blocks.read().contains(&id)
    }

    pub fn network_debug_snapshot(&self) -> NetworkDebugSnapshot {
        self.debug.read().clone()
    }

    pub fn client_debug_snapshot(&self) -> ClientDebugSnapshot {
        self.client_debug.read().clone()
    }

    pub fn cached_blocks(&self) -> Vec<CachedBlock> {
        self.cached_blocks.read().values().cloned().collect()
    }

    pub fn cached_block(&self, id: Uuid) -> Option<CachedBlock> {
        self.cached_blocks.read().get(&id).cloned()
    }

    pub fn cache_references(&self, list: BlockReferenceList) {
        self.send(WorkerCommand::CacheReferences(list));
    }

    pub fn pause_sending(&self) {
        self.send(WorkerCommand::PauseSending);
    }

    pub fn step_sending(&self) {
        self.send(WorkerCommand::StepSending);
    }

    pub fn resume_sending(&self) {
        self.send(WorkerCommand::ResumeSending);
    }

    fn send(&self, command: WorkerCommand) {
        self.commands
            .send(command)
            .unwrap_or_else(|_| fatal("block client worker stopped"));
    }
}

impl Drop for BlockClient {
    fn drop(&mut self) {
        self.shutdown.request();
    }
}

pub struct BlockHandle<B: Block> {
    client_id: Uuid,
    id: Uuid,
    block: Arc<TypedBlock<B>>,
    commands: CommandSender,
    access: Arc<RwLock<()>>,
}

struct AppliedCrdtOperation<O> {
    operation: O,
    implicit_name: String,
    references: ReferenceDelta,
}

pub struct CrdtOperationTransaction<'a, B: Block> {
    current: &'a mut B,
    workspace_id: Uuid,
    applied: Vec<AppliedCrdtOperation<B::Operation>>,
}

impl<B: Block> CrdtOperationTransaction<'_, B> {
    pub fn current(&self) -> &B {
        self.current
    }

    pub fn apply(&mut self, operation: B::Operation) {
        let before =
            normalized_references(self.current.references_for_workspace(self.workspace_id));
        B::apply_operation(self.current, &operation);
        let implicit_name = self.current.implicit_name();
        let after = normalized_references(self.current.references_for_workspace(self.workspace_id));
        self.applied.push(AppliedCrdtOperation {
            operation,
            implicit_name,
            references: reference_delta(&before, &after),
        });
    }
}

impl<B: Block> BlockHistoryTransaction<B> for CrdtOperationTransaction<'_, B> {
    fn current(&self) -> &B {
        self.current()
    }

    fn apply(&mut self, operation: B::Operation) {
        self.apply(operation);
    }
}

#[derive(Clone)]
pub struct HistoryMetadata {
    value: Arc<dyn Any + Send + Sync>,
    bytes: usize,
}

impl HistoryMetadata {
    pub fn new<T: Any + Send + Sync>(value: T, bytes: usize) -> Self {
        Self {
            value: Arc::new(value),
            bytes,
        }
    }

    pub fn downcast<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        Arc::clone(&self.value).downcast().ok()
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockRelationships {
    pub parent: BlockParent,
    pub references: Vec<Uuid>,
    pub backrefs: Vec<Uuid>,
}

impl<B: Block> Clone for BlockHandle<B> {
    fn clone(&self) -> Self {
        Self {
            client_id: self.client_id,
            id: self.id,
            block: Arc::clone(&self.block),
            commands: self.commands.clone(),
            access: Arc::clone(&self.access),
        }
    }
}

impl<B: Block> BlockHandle<B> {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn author(&self) -> Option<Uuid> {
        *self.block.author.read()
    }

    pub fn read(&self) -> Option<BlockReadGuard<'_, B>> {
        let access = self.access.read();
        let guard = self.block.shared.value.read();
        if guard.is_none() {
            return None;
        }
        Some(BlockReadGuard {
            _access: access,
            guard: RwLockReadGuard::map(guard, |value| value.as_ref().unwrap()),
        })
    }

    pub fn operate(&self, operation: B::Operation) {
        self.block
            .local_operations(vec![operation], HistoryMode::Standalone, None);
        self.commands
            .send(WorkerCommand::Operate { id: self.id })
            .unwrap_or_else(|_| fatal("block client worker stopped"));
    }

    pub fn replace(&self, replacement: B) {
        self.block.local_replace(replacement);
        self.commands
            .send(WorkerCommand::Operate { id: self.id })
            .unwrap_or_else(|_| fatal("block client worker stopped"));
    }

    pub fn dynamic_artifact(&self) -> Option<DynamicArtifactDescriptor> {
        self.block.dynamic_artifact.read().clone()
    }

    pub fn revision(&self) -> u64 {
        self.block.revision.load(Ordering::Relaxed)
    }

    pub fn operate_grouped(&self, operations: impl IntoIterator<Item = B::Operation>) {
        self.operate_grouped_with_history_metadata(operations, None);
    }

    pub fn operate_grouped_with_history_metadata(
        &self,
        operations: impl IntoIterator<Item = B::Operation>,
        metadata: Option<HistoryMetadata>,
    ) {
        let operations = operations.into_iter().collect::<Vec<_>>();
        if operations.is_empty() {
            return;
        }
        self.block
            .local_operations(operations, HistoryMode::Grouped, metadata);
        self.commands
            .send(WorkerCommand::Operate { id: self.id })
            .unwrap_or_else(|_| fatal("block client worker stopped"));
    }

    pub fn edit_crdt_grouped_with_history_metadata<R>(
        &self,
        metadata: Option<HistoryMetadata>,
        edit: impl FnOnce(&mut CrdtOperationTransaction<'_, B>) -> R,
    ) -> R {
        let (result, changed) = self
            .block
            .local_crdt_edit(HistoryMode::Grouped, metadata, edit);
        if changed {
            self.commands
                .send(WorkerCommand::Operate { id: self.id })
                .unwrap_or_else(|_| fatal("block client worker stopped"));
        }
        result
    }

    pub fn finish_history_group(&self) {
        self.block.history.write().finish_group();
    }

    pub fn supports_history(&self) -> bool {
        B::History::ENABLED
    }

    pub fn can_undo(&self) -> bool {
        self.block.history.read().can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.block.history.read().can_redo()
    }

    pub fn undo(&self) {
        self.undo_with_history_metadata();
    }

    pub fn undo_with_history_metadata(&self) -> Option<HistoryMetadata> {
        let result = self.block.apply_history(HistoryDirection::Undo);
        if result.applied {
            self.commands
                .send(WorkerCommand::Operate { id: self.id })
                .unwrap_or_else(|_| fatal("block client worker stopped"));
        }
        result.metadata
    }

    pub fn redo(&self) {
        self.redo_with_history_metadata();
    }

    pub fn redo_with_history_metadata(&self) -> Option<HistoryMetadata> {
        let result = self.block.apply_history(HistoryDirection::Redo);
        if result.applied {
            self.commands
                .send(WorkerCommand::Operate { id: self.id })
                .unwrap_or_else(|_| fatal("block client worker stopped"));
        }
        result.metadata
    }

    pub fn set_parent(&self, parent: BlockParent) {
        if let BlockParent::Uuid(id) = parent {
            let mut relationships = self.block.relationships.write();
            if !relationships.backrefs.contains(&id) {
                relationships.backrefs.push(id);
            }
        }
        self.commands
            .send(WorkerCommand::SetBlockParent {
                id: self.id,
                parent,
            })
            .unwrap_or_else(|_| fatal("block client worker stopped"));
    }

    pub fn name(&self) -> String {
        self.block.name.read().clone()
    }

    pub fn set_name(&self, name: impl Into<String>) {
        self.commands
            .send(WorkerCommand::SetBlockName {
                id: self.id,
                name: name.into(),
            })
            .unwrap_or_else(|_| fatal("block client worker stopped"));
    }

    pub fn relationships(&self) -> BlockRelationships {
        self.block.relationships.read().clone()
    }

    pub async fn loaded(&self) {
        let mut loaded = self.block.loaded.subscribe();
        while !*loaded.borrow_and_update() {
            loaded
                .changed()
                .await
                .unwrap_or_else(|_| fatal("block client worker stopped before loading the block"));
        }
    }

    pub async fn wait_until(&self, mut predicate: impl FnMut(&B) -> bool) {
        let mut changed = self.block.changed.subscribe();
        loop {
            if self.read().is_some_and(|block| predicate(&block)) {
                return;
            }
            changed
                .changed()
                .await
                .unwrap_or_else(|_| fatal("block client worker stopped while waiting for a block"));
        }
    }
}

pub trait BlockHistoryHandle {
    fn supports_history(&self) -> bool;
    fn can_undo(&self) -> bool;
    fn can_redo(&self) -> bool;
    fn undo(&self);
    fn redo(&self);
}

pub trait BlockHandleAccess {
    fn id(&self) -> Uuid;
    fn block_type(&self) -> Uuid;
    fn name(&self) -> String;
    fn relationships(&self) -> Option<BlockRelationships>;
    fn set_parent(&self, parent: BlockParent);
    fn history(&self) -> Option<&dyn BlockHistoryHandle>;
}

impl<B: Block> BlockHandleAccess for BlockHandle<B> {
    fn id(&self) -> Uuid {
        BlockHandle::id(self)
    }

    fn block_type(&self) -> Uuid {
        B::TYPE_ID
    }

    fn name(&self) -> String {
        BlockHandle::name(self)
    }

    fn relationships(&self) -> Option<BlockRelationships> {
        self.read().map(|_| BlockHandle::relationships(self))
    }

    fn set_parent(&self, parent: BlockParent) {
        BlockHandle::set_parent(self, parent);
    }

    fn history(&self) -> Option<&dyn BlockHistoryHandle> {
        self.supports_history().then_some(self)
    }
}

impl<B: Block> BlockHistoryHandle for BlockHandle<B> {
    fn supports_history(&self) -> bool {
        BlockHandle::supports_history(self)
    }

    fn can_undo(&self) -> bool {
        BlockHandle::can_undo(self)
    }

    fn can_redo(&self) -> bool {
        BlockHandle::can_redo(self)
    }

    fn undo(&self) {
        BlockHandle::undo(self);
    }

    fn redo(&self) {
        BlockHandle::redo(self);
    }
}

pub struct ReferenceList {
    list: BlockReferenceList,
    shared: Arc<ReferenceListShared>,
    commands: CommandSender,
    watched_reference_lists: Arc<RwLock<HashMap<BlockReferenceList, Weak<ReferenceListShared>>>>,
}

impl ReferenceList {
    pub fn read(&self) -> Vec<BlockReference> {
        self.shared.blocks.read().clone()
    }

    pub fn is_loaded(&self) -> bool {
        *self.shared.loaded.borrow()
    }
}

impl Drop for ReferenceList {
    fn drop(&mut self) {
        if self.shared.subscribers.fetch_sub(1, Ordering::AcqRel) != 1 {
            return;
        }
        let mut watched = self.watched_reference_lists.write();
        let is_current = watched
            .get(&self.list)
            .and_then(Weak::upgrade)
            .is_some_and(|shared| Arc::ptr_eq(&shared, &self.shared));
        if is_current {
            watched.remove(&self.list);
            let _ = self
                .commands
                .send(WorkerCommand::UnwatchReferences(self.list));
        }
    }
}

struct ReferenceListShared {
    blocks: RwLock<Vec<BlockReference>>,
    loaded: watch::Sender<bool>,
    subscribers: AtomicUsize,
}

pub struct BlockReadGuard<'a, B: Block> {
    _access: RwLockReadGuard<'a, ()>,
    guard: MappedRwLockReadGuard<'a, B>,
}

impl<B: Block> Deref for BlockReadGuard<'_, B> {
    type Target = B;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

pub struct BlockBatch<'a> {
    client_id: Uuid,
    commands: CommandSender,
    _access: RwLockWriteGuard<'a, ()>,
    ids: Vec<Uuid>,
}

impl BlockBatch<'_> {
    pub fn operate<B: Block>(&mut self, block: &BlockHandle<B>, operation: B::Operation) {
        if block.client_id != self.client_id {
            fatal("cannot batch a block owned by another client");
        }
        if self.ids.contains(&block.id) {
            fatal("a batch may contain at most one operation per block");
        }
        block
            .block
            .local_operations(vec![operation], HistoryMode::Standalone, None);
        self.ids.push(block.id);
    }
}

impl Drop for BlockBatch<'_> {
    fn drop(&mut self) {
        if self.ids.is_empty() {
            return;
        }
        self.commands
            .send(WorkerCommand::OperateBatch {
                ids: std::mem::take(&mut self.ids),
            })
            .unwrap_or_else(|_| fatal("block client worker stopped"));
    }
}

struct BlockShared<B: Block> {
    value: RwLock<Option<B>>,
}

enum WorkerCommand {
    Connect {
        url: String,
        account_id: Uuid,
        workspace_id: Uuid,
    },
    AddBlock(Arc<dyn ErasedBlock>),
    Operate {
        id: Uuid,
    },
    OperateBatch {
        ids: Vec<Uuid>,
    },
    SetBlockParent {
        id: Uuid,
        parent: BlockParent,
    },
    SetBlockName {
        id: Uuid,
        name: String,
    },
    ListReferences {
        list: BlockReferenceList,
        completed: oneshot::Sender<Vec<BlockReference>>,
    },
    WatchReferences {
        list: BlockReferenceList,
        shared: Arc<ReferenceListShared>,
    },
    CacheReferences(BlockReferenceList),
    UnwatchReferences(BlockReferenceList),
    ListBlockAccess {
        id: Uuid,
        completed: oneshot::Sender<Result<Vec<BlockAccessEntry>, String>>,
    },
    SetBlockAccess {
        id: Uuid,
        account_id: Uuid,
        access: BlockAccess,
    },
    Synchronize(oneshot::Sender<()>),
    PauseSending,
    StepSending,
    ResumeSending,
}

async fn worker_main(
    mut commands: mpsc::UnboundedReceiver<WorkerCommand>,
    shutdown: Shutdown,
    access: Arc<RwLock<()>>,
    debug: Arc<RwLock<NetworkDebugSnapshot>>,
    client_debug: Arc<RwLock<ClientDebugSnapshot>>,
    cached_blocks: Arc<RwLock<HashMap<Uuid, CachedBlock>>>,
    denied_blocks: Arc<RwLock<HashSet<Uuid>>>,
) {
    let mut state = WorkerState::new(access, debug, client_debug, cached_blocks, denied_blocks);
    while let Some(command) = commands.next().await {
        match command {
            WorkerCommand::Connect {
                url,
                account_id,
                workspace_id,
            } => {
                run_connected(
                    url,
                    account_id,
                    workspace_id,
                    &shutdown,
                    &mut state,
                    &mut commands,
                )
                .await;
                return;
            }
            command => state.handle_command(command),
        }
    }
}

/// The websocket endpoint for a server URL. The blocks websocket shares the
/// server's host and scheme with the management endpoint, and carries the
/// connection's identity in its query string.
fn websocket_url(url: &str, account_id: Uuid, workspace_id: Uuid) -> String {
    let base = match url.split_once("://") {
        Some(("http", host)) => format!("ws://{host}"),
        Some(("https", host)) => format!("wss://{host}"),
        _ => url.to_owned(),
    };
    format!("{base}/?{ACCOUNT_PARAMETER}={account_id}&{WORKSPACE_PARAMETER}={workspace_id}")
}

/// Runs the connection until it ends. There is no reconnecting, so every way
/// out of here stops the worker for good.
async fn run_connected(
    url: String,
    account_id: Uuid,
    workspace_id: Uuid,
    shutdown: &Shutdown,
    state: &mut WorkerState,
    commands: &mut mpsc::UnboundedReceiver<WorkerCommand>,
) {
    let url = websocket_url(&url, account_id, workspace_id);
    let mut socket = match Socket::connect(&url).await {
        Ok(socket) => socket,
        Err(error) => return stop_or_fatal(shutdown, error),
    };
    state.connected = true;
    state.queue_initial_requests();

    loop {
        while state.can_send() {
            let Some(message) = state.outbound.pop_front() else {
                break;
            };
            let text = serde_json::to_string(&message).unwrap_or_else(|error| {
                fatal(format!("failed to serialize client message: {error}"))
            });
            if let Err(error) = socket.send_text(text.clone()).await {
                return stop_or_fatal(shutdown, error);
            }
            state.message_sent(text);
        }
        state.refresh_debug();

        futures_util::select! {
            command = commands.next() => {
                let Some(command) = command else {
                    return;
                };
                if matches!(command, WorkerCommand::Connect { .. }) {
                    fatal("BlockClient::connect may only be called once");
                }
                state.handle_command(command);
                state.finish_synchronization();
            }
            message = socket.next().fuse() => {
                let message = match message {
                    Some(Ok(message)) => message,
                    Some(Err(error)) => return stop_or_fatal(shutdown, error),
                    None => return stop_or_fatal(shutdown, "block server connection closed"),
                };
                match message {
                    SocketMessage::Text(text) => {
                        state.record_traffic(NetworkDirection::Received, text.clone());
                        let message: ServerMessage = serde_json::from_str(&text)
                            .unwrap_or_else(|error| fatal(format!("invalid server message: {error}: {text}")));
                        state.handle_server_message(message);
                        state.finish_synchronization();
                    }
                    SocketMessage::Ping(length) => {
                        state.record_traffic(
                            NetworkDirection::Received,
                            format!("<websocket ping: {length} bytes>"),
                        );
                        state.record_traffic(NetworkDirection::Sent, "<websocket pong>".into());
                    }
                    SocketMessage::Close => {
                        return stop_or_fatal(shutdown, "block server connection closed")
                    }
                }
            }
        }
    }
}

/// Ends the connection. A client that is still running cannot carry on without
/// one, so losing it is fatal; once the client is gone there is nobody left for
/// the failure to matter to.
fn stop_or_fatal(shutdown: &Shutdown, error: impl AsRef<str>) {
    if shutdown.requested() {
        return;
    }
    fatal(error)
}

struct WorkerState {
    connected: bool,
    access: Arc<RwLock<()>>,
    blocks: HashMap<Uuid, Arc<dyn ErasedBlock>>,
    reference_lists: HashMap<BlockReferenceList, Arc<ReferenceListShared>>,
    requests: HashMap<Uuid, PendingRequest>,
    outbound: VecDeque<ClientMessage>,
    deferred: VecDeque<DeferredRequest>,
    synchronization_waiters: Vec<oneshot::Sender<()>>,
    sending_paused: bool,
    steps_remaining: usize,
    debug: Arc<RwLock<NetworkDebugSnapshot>>,
    client_debug: Arc<RwLock<ClientDebugSnapshot>>,
    cached_blocks: Arc<RwLock<HashMap<Uuid, CachedBlock>>>,
    denied_blocks: Arc<RwLock<HashSet<Uuid>>>,
}

impl WorkerState {
    fn new(
        access: Arc<RwLock<()>>,
        debug: Arc<RwLock<NetworkDebugSnapshot>>,
        client_debug: Arc<RwLock<ClientDebugSnapshot>>,
        cached_blocks: Arc<RwLock<HashMap<Uuid, CachedBlock>>>,
        denied_blocks: Arc<RwLock<HashSet<Uuid>>>,
    ) -> Self {
        Self {
            connected: false,
            access,
            blocks: HashMap::new(),
            reference_lists: HashMap::new(),
            requests: HashMap::new(),
            outbound: VecDeque::new(),
            deferred: VecDeque::new(),
            synchronization_waiters: Vec::new(),
            sending_paused: false,
            steps_remaining: 0,
            debug,
            client_debug,
            cached_blocks,
            denied_blocks,
        }
    }

    fn cache_block(&self, block: CachedBlock) {
        self.cached_blocks.write().insert(block.id, block);
    }

    fn cache_reference_blocks(&self, blocks: &[BlockReference]) {
        let mut cache = self.cached_blocks.write();
        for block in blocks {
            cache.insert(
                block.id,
                CachedBlock {
                    id: block.id,
                    block_type: block.block_type,
                    author: block.author,
                    name: block.name.clone(),
                },
            );
        }
    }

    fn cache_registered_block(&self, id: Uuid) {
        let block = &self.blocks[&id];
        self.cache_block(CachedBlock {
            id,
            block_type: block.block_type_id(),
            author: block.author().expect("registered block omitted author"),
            name: block.name(),
        });
    }

    fn handle_command(&mut self, command: WorkerCommand) {
        match command {
            WorkerCommand::Connect { .. } => fatal("unexpected connect command"),
            WorkerCommand::AddBlock(block) => {
                let id = block.id();
                if self.blocks.insert(id, Arc::clone(&block)).is_some() {
                    fatal(format!("block {id} is already registered"));
                }
                if self.connected {
                    self.queue_initial_request(block);
                }
            }
            WorkerCommand::Operate { id } => {
                if !self.blocks.contains_key(&id) {
                    fatal(format!("unknown block {id}"));
                }
                self.maybe_send_update(id);
            }
            WorkerCommand::OperateBatch { ids } => {
                if ids.iter().any(|id| !self.blocks.contains_key(id)) {
                    fatal("batch contains an unknown block");
                }
                self.maybe_send_batch(ids);
            }
            WorkerCommand::SetBlockParent { id, parent } => {
                self.deferred
                    .push_back(DeferredRequest::SetBlockParent { id, parent });
            }
            WorkerCommand::SetBlockName { id, name } => {
                self.deferred
                    .push_back(DeferredRequest::SetBlockName { id, name });
            }
            WorkerCommand::ListReferences { list, completed } => {
                self.deferred
                    .push_back(DeferredRequest::ListReferences { list, completed });
            }
            WorkerCommand::WatchReferences { list, shared } => {
                if self.reference_lists.insert(list, shared).is_some() {
                    fatal("this reference list is already being watched");
                }
                self.deferred
                    .push_back(DeferredRequest::WatchReferences { list });
            }
            WorkerCommand::CacheReferences(list) => {
                self.deferred
                    .push_back(DeferredRequest::CacheReferences { list });
            }
            WorkerCommand::UnwatchReferences(list) => {
                self.reference_lists.remove(&list);
                self.deferred
                    .push_back(DeferredRequest::UnwatchReferences { list });
            }
            WorkerCommand::ListBlockAccess { id, completed } => {
                self.deferred
                    .push_back(DeferredRequest::ListBlockAccess { id, completed });
            }
            WorkerCommand::SetBlockAccess {
                id,
                account_id,
                access,
            } => {
                self.deferred.push_back(DeferredRequest::SetBlockAccess {
                    id,
                    account_id,
                    access,
                });
            }
            WorkerCommand::Synchronize(completed) => {
                self.synchronization_waiters.push(completed);
            }
            WorkerCommand::PauseSending => {
                self.sending_paused = true;
                self.steps_remaining = 0;
            }
            WorkerCommand::StepSending => {
                if self.sending_paused {
                    self.steps_remaining = self.steps_remaining.saturating_add(1);
                }
            }
            WorkerCommand::ResumeSending => {
                self.sending_paused = false;
                self.steps_remaining = 0;
            }
        }
        self.refresh_debug();
    }

    fn can_send(&self) -> bool {
        !self.sending_paused || self.steps_remaining > 0
    }

    fn message_sent(&mut self, payload: String) {
        if self.sending_paused {
            self.steps_remaining = self.steps_remaining.saturating_sub(1);
        }
        self.record_traffic(NetworkDirection::Sent, payload);
    }

    fn record_traffic(&self, direction: NetworkDirection, payload: String) {
        self.debug.write().traffic.push(NetworkTrafficEntry {
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            direction,
            payload,
        });
    }

    fn refresh_debug(&self) {
        let changes_saved = !self.has_unsaved_changes();
        let mut debug = self.debug.write();
        debug.sending_paused = self.sending_paused;
        debug.queued_messages = self.outbound.len();
        debug.changes_saved = changes_saved;
        drop(debug);

        let mut blocks: Vec<_> = self
            .blocks
            .values()
            .map(|block| block.debug_snapshot())
            .collect();
        blocks.sort_by_key(|block| block.id);
        let reference_lists: Vec<_> = self
            .reference_lists
            .iter()
            .map(|(list, shared)| ReferenceListDebugSnapshot {
                list: *list,
                loaded: *shared.loaded.borrow(),
                blocks: shared.blocks.read().len(),
            })
            .collect();
        let cached_blocks: Vec<_> = self.cached_blocks.read().values().cloned().collect();

        let mut pending_requests: Vec<_> = self
            .requests
            .iter()
            .map(|(request_id, request)| request.debug_snapshot(*request_id))
            .collect();
        pending_requests.sort_by_key(|request| request.request_id);
        let outbound_messages = self
            .outbound
            .iter()
            .map(client_message_debug_entry)
            .collect();
        let deferred_requests = self
            .deferred
            .iter()
            .map(DeferredRequest::debug_entry)
            .collect();

        let mut client_debug = self.client_debug.write();
        client_debug.connected = self.connected;
        client_debug.sending_paused = self.sending_paused;
        client_debug.queued_messages = self.outbound.len();
        client_debug.steps_remaining = self.steps_remaining;
        client_debug.changes_saved = changes_saved;
        client_debug.synchronization_waiters = self.synchronization_waiters.len();
        client_debug.blocks = blocks;
        client_debug.reference_lists = reference_lists;
        client_debug.cached_blocks = cached_blocks;
        client_debug.pending_requests = pending_requests;
        client_debug.outbound_messages = outbound_messages;
        client_debug.deferred_requests = deferred_requests;
    }

    fn has_unsaved_changes(&self) -> bool {
        self.blocks.values().any(|block| block.has_local_changes())
            || self.requests.values().any(PendingRequest::changes_data)
            || self.outbound.iter().any(client_message_changes_data)
            || self.deferred.iter().any(DeferredRequest::changes_data)
    }

    fn queue_initial_requests(&mut self) {
        let blocks: Vec<_> = self.blocks.values().cloned().collect();
        for block in blocks {
            self.queue_initial_request(block);
        }
    }

    fn queue_initial_request(&mut self, block: Arc<dyn ErasedBlock>) {
        let request_id = Uuid::new_v4();
        let id = block.id();
        let message = if let Some(data) = block.initial_data() {
            self.requests
                .insert(request_id, PendingRequest::Create { id });
            ClientMessage::CreateBlock {
                request_id,
                id,
                block_type: block.block_type_id(),
                data,
                implicit_name: block.initial_name(),
                references: block.initial_references(),
                watch: true,
            }
        } else {
            self.requests
                .insert(request_id, PendingRequest::Read { id });
            ClientMessage::ReadBlock {
                request_id,
                id,
                watch: true,
            }
        };
        self.outbound.push_back(message);
    }

    fn maybe_send_update(&mut self, id: Uuid) {
        if !self.connected {
            return;
        }
        loop {
            let block = self.blocks.get(&id).unwrap();
            let Some(update) = block.next_update() else {
                return;
            };
            let request_id = Uuid::new_v4();
            self.requests.insert(
                request_id,
                PendingRequest::Update {
                    id,
                    operation_id: update.operation_id,
                },
            );
            self.outbound.push_back(ClientMessage::UpdateBlock {
                request_id,
                id,
                seq: update.seq,
                operation_id: update.operation_id,
                operation: update.operation,
                implicit_name: update.implicit_name,
                references: update.references,
            });
        }
    }

    fn maybe_send_batch(&mut self, ids: Vec<Uuid>) {
        if !self.connected {
            return;
        }
        let mut updates = Vec::new();
        for id in ids {
            if let Some(update) = self.blocks[&id].next_update() {
                updates.push(BlockUpdate {
                    id,
                    seq: update.seq,
                    operation_id: update.operation_id,
                    operation: update.operation,
                    implicit_name: update.implicit_name,
                    references: update.references,
                });
            }
        }
        if updates.is_empty() {
            return;
        }
        let request_id = Uuid::new_v4();
        self.requests.insert(
            request_id,
            PendingRequest::Batch {
                operations: updates
                    .iter()
                    .map(|update| (update.id, update.operation_id))
                    .collect(),
            },
        );
        self.outbound.push_back(ClientMessage::UpdateBatch {
            request_id,
            updates,
        });
    }

    fn handle_server_message(&mut self, message: ServerMessage) {
        match message {
            ServerMessage::Ok {
                request_id,
                command,
                id,
                seq,
                operation_id,
            } => {
                let pending = self
                    .requests
                    .remove(&request_id)
                    .unwrap_or_else(|| fatal(format!("response for unknown request {request_id}")));
                match (pending, command) {
                    (PendingRequest::Create { id: expected }, CommandKind::CreateBlock)
                        if expected == id =>
                    {
                        self.blocks[&id].created();
                        self.cache_registered_block(id);
                        self.maybe_send_update(id);
                    }
                    (
                        PendingRequest::Update {
                            id: expected,
                            operation_id: expected_operation,
                        },
                        CommandKind::UpdateBlock,
                    ) if expected == id && Some(expected_operation) == operation_id => {
                        let seq =
                            seq.unwrap_or_else(|| fatal("update acknowledgement omitted seq"));
                        self.blocks[&id].acknowledge(expected_operation, seq);
                        self.maybe_send_update(id);
                    }
                    (
                        PendingRequest::SetBlockParent {
                            id: expected,
                            parent,
                        },
                        CommandKind::SetBlockParent,
                    ) if expected == id => {
                        if let Some(block) = self.blocks.get(&id) {
                            block.set_parent(parent);
                        }
                    }
                    (
                        PendingRequest::SetBlockName { id: expected, name },
                        CommandKind::SetBlockName,
                    ) if expected == id => {
                        if let Some(block) = self.cached_blocks.write().get_mut(&id) {
                            block.name = name;
                        }
                    }
                    (PendingRequest::UnwatchReferences, CommandKind::UnwatchReferences)
                        if id.is_nil() => {}
                    (
                        PendingRequest::SetBlockAccess { id: expected },
                        CommandKind::SetBlockAccess,
                    ) if expected == id => {}
                    _ => fatal("server response did not match its request"),
                }
            }
            ServerMessage::ReadBlock {
                request_id,
                command,
                id,
                block_type,
                author,
                snapshot,
                snapshot_seq,
                operations,
                parent,
                name,
            } => {
                match (self.requests.remove(&request_id), command) {
                    (Some(PendingRequest::Read { id: expected }), CommandKind::ReadBlock)
                        if expected == id => {}
                    _ => fatal("read response did not match its request"),
                }
                let block = &self.blocks[&id];
                if block.block_type_id() != block_type {
                    fatal(format!(
                        "block {id} has type {block_type}, expected {}",
                        block.block_type_id()
                    ));
                }
                block.resolve_authored(
                    author,
                    snapshot,
                    snapshot_seq,
                    operations,
                    parent,
                    name.clone(),
                );
                self.cache_block(CachedBlock {
                    id,
                    block_type,
                    author,
                    name,
                });
                self.maybe_send_update(id);
            }
            ServerMessage::BatchOk {
                request_id,
                command: CommandKind::UpdateBatch,
                operations,
            } => {
                let pending = self
                    .requests
                    .remove(&request_id)
                    .unwrap_or_else(|| fatal(format!("response for unknown request {request_id}")));
                let PendingRequest::Batch {
                    operations: expected,
                } = pending
                else {
                    fatal("batch response did not match its request");
                };
                if expected.len() != operations.len() {
                    fatal("batch response operation count mismatch");
                }
                for ((expected_id, expected_operation), operation) in
                    expected.into_iter().zip(operations)
                {
                    if operation.id != expected_id
                        || operation.operation.operation_id != expected_operation
                    {
                        fatal("batch response operation mismatch");
                    }
                    self.blocks[&operation.id]
                        .acknowledge(operation.operation.operation_id, operation.operation.seq);
                    self.maybe_send_update(operation.id);
                }
            }
            ServerMessage::BatchOk { .. } => fatal("invalid batch response command"),
            ServerMessage::Error {
                request_id,
                command,
                id: response_id,
                code,
                message,
                expected_seq,
            } => {
                if code == ErrorCode::InvalidSeq && command == Some(CommandKind::UpdateBlock) {
                    let request_id =
                        request_id.unwrap_or_else(|| fatal("sequence error omitted request id"));
                    let pending = self
                        .requests
                        .remove(&request_id)
                        .unwrap_or_else(|| fatal("sequence error referenced unknown request"));
                    let PendingRequest::Update { id, operation_id } = pending else {
                        fatal("sequence error referenced a non-update request");
                    };
                    if Some(id) != response_id {
                        fatal("sequence error block mismatch");
                    }
                    let retry_now = self.blocks[&id].sequence_conflict(
                        operation_id,
                        expected_seq
                            .unwrap_or_else(|| fatal("sequence error omitted expected_seq")),
                    );
                    if retry_now {
                        self.maybe_send_update(id);
                    }
                    return;
                }
                if code == ErrorCode::PermissionDenied {
                    // Permissions can be withdrawn while a block is open, so a
                    // refusal is an expected answer rather than a protocol fault.
                    self.deny_access(request_id, response_id, message);
                    return;
                }
                fatal(format!(
                    "server rejected request {:?} for {:?}: {:?}: {}",
                    request_id, response_id, code, message
                ));
            }
            ServerMessage::BlockUpdated {
                id,
                name,
                operation,
            } => {
                let access = Arc::clone(&self.access);
                let _access = access.write();
                let block = self
                    .blocks
                    .get(&id)
                    .unwrap_or_else(|| fatal(format!("update for unknown block {id}")));
                block.remote_operation(operation);
                block.set_name(name.clone());
                if let Some(cached) = self.cached_blocks.write().get_mut(&id) {
                    cached.name = name;
                }
                drop(_access);
                self.maybe_send_update(id);
            }
            ServerMessage::BatchUpdated { operations } => {
                let access = Arc::clone(&self.access);
                let _access = access.write();
                let mut ids = Vec::with_capacity(operations.len());
                for BlockOperation {
                    id,
                    name,
                    operation,
                } in operations
                {
                    let block = self
                        .blocks
                        .get(&id)
                        .unwrap_or_else(|| fatal(format!("update for unknown block {id}")));
                    block.remote_operation(operation);
                    block.set_name(name.clone());
                    if let Some(cached) = self.cached_blocks.write().get_mut(&id) {
                        cached.name = name;
                    }
                    ids.push(id);
                }
                drop(_access);
                for id in ids {
                    self.maybe_send_update(id);
                }
            }
            ServerMessage::Presence { .. } => {}
            ServerMessage::BlockNameUpdated { id, name } => {
                if let Some(block) = self.blocks.get(&id) {
                    block.set_name(name.clone());
                }
                if let Some(cached) = self.cached_blocks.write().get_mut(&id) {
                    cached.name = name;
                }
            }
            ServerMessage::References {
                request_id,
                list,
                blocks,
            } => {
                self.cache_reference_blocks(&blocks);
                let pending = self
                    .requests
                    .remove(&request_id)
                    .unwrap_or_else(|| fatal("reference response referenced an unknown request"));
                match pending {
                    PendingRequest::ListReferences {
                        list: expected,
                        completed,
                    } if expected == list => {
                        let _ = completed.send(blocks);
                    }
                    PendingRequest::WatchReferences { list: expected } if expected == list => {
                        if let Some(shared) = self.reference_lists.get(&list) {
                            *shared.blocks.write() = blocks;
                            shared.loaded.send_replace(true);
                        }
                    }
                    PendingRequest::CacheReferences { list: expected } if expected == list => {}
                    _ => fatal("reference response did not match its request"),
                }
            }
            ServerMessage::ReferencesUpdated { list, blocks } => {
                self.cache_reference_blocks(&blocks);
                if let Some(shared) = self.reference_lists.get(&list) {
                    *shared.blocks.write() = blocks;
                }
            }
            ServerMessage::BlockAccessList {
                request_id,
                command,
                id,
                entries,
            } => {
                match (self.requests.remove(&request_id), command) {
                    (
                        Some(PendingRequest::ListBlockAccess {
                            id: expected,
                            completed,
                        }),
                        CommandKind::ListBlockAccess,
                    ) if expected == id => {
                        let _ = completed.send(Ok(entries));
                    }
                    _ => fatal("block access response did not match its request"),
                };
            }
        }
    }

    /// Records that the server refused a request, dropping it so the client
    /// keeps running with the block marked inaccessible.
    fn deny_access(&mut self, request_id: Option<Uuid>, id: Option<Uuid>, message: String) {
        let pending = request_id.and_then(|request_id| self.requests.remove(&request_id));
        if let Some(PendingRequest::ListBlockAccess { completed, .. }) = pending {
            let _ = completed.send(Err(message));
        }
        if let Some(id) = id {
            self.denied_blocks.write().insert(id);
        }
    }

    fn finish_synchronization(&mut self) {
        self.maybe_send_deferred();
        if !self.connected
            || !self.requests.is_empty()
            || !self.outbound.is_empty()
            || !self.deferred.is_empty()
            || self.blocks.values().any(|block| !block.is_synchronized())
        {
            return;
        }

        for completed in self.synchronization_waiters.drain(..) {
            let _ = completed.send(());
        }
    }

    fn maybe_send_deferred(&mut self) {
        if !self.connected
            || !self.requests.is_empty()
            || !self.outbound.is_empty()
            || self.blocks.values().any(|block| !block.is_synchronized())
        {
            return;
        }
        let Some(request) = self.deferred.pop_front() else {
            return;
        };
        let request_id = Uuid::new_v4();
        match request {
            DeferredRequest::SetBlockParent { id, parent } => {
                self.requests
                    .insert(request_id, PendingRequest::SetBlockParent { id, parent });
                self.outbound.push_back(ClientMessage::SetBlockParent {
                    request_id,
                    id,
                    parent,
                });
            }
            DeferredRequest::SetBlockName { id, name } => {
                self.requests.insert(
                    request_id,
                    PendingRequest::SetBlockName {
                        id,
                        name: name.clone(),
                    },
                );
                self.outbound.push_back(ClientMessage::SetBlockName {
                    request_id,
                    id,
                    name,
                });
            }
            DeferredRequest::ListReferences { list, completed } => {
                self.requests.insert(
                    request_id,
                    PendingRequest::ListReferences { list, completed },
                );
                self.outbound.push_back(ClientMessage::ListReferences {
                    request_id,
                    list,
                    watch: false,
                });
            }
            DeferredRequest::WatchReferences { list } => {
                self.requests
                    .insert(request_id, PendingRequest::WatchReferences { list });
                self.outbound.push_back(ClientMessage::ListReferences {
                    request_id,
                    list,
                    watch: true,
                });
            }
            DeferredRequest::CacheReferences { list } => {
                self.requests
                    .insert(request_id, PendingRequest::CacheReferences { list });
                self.outbound.push_back(ClientMessage::ListReferences {
                    request_id,
                    list,
                    watch: false,
                });
            }
            DeferredRequest::UnwatchReferences { list } => {
                self.requests
                    .insert(request_id, PendingRequest::UnwatchReferences);
                self.outbound
                    .push_back(ClientMessage::UnwatchReferences { request_id, list });
            }
            DeferredRequest::ListBlockAccess { id, completed } => {
                self.requests.insert(
                    request_id,
                    PendingRequest::ListBlockAccess { id, completed },
                );
                self.outbound
                    .push_back(ClientMessage::ListBlockAccess { request_id, id });
            }
            DeferredRequest::SetBlockAccess {
                id,
                account_id,
                access,
            } => {
                self.requests
                    .insert(request_id, PendingRequest::SetBlockAccess { id });
                self.outbound.push_back(ClientMessage::SetBlockAccess {
                    request_id,
                    id,
                    account_id,
                    access,
                });
            }
        }
    }
}

enum PendingRequest {
    Create {
        id: Uuid,
    },
    Read {
        id: Uuid,
    },
    Update {
        id: Uuid,
        operation_id: Uuid,
    },
    Batch {
        operations: Vec<(Uuid, Uuid)>,
    },
    SetBlockParent {
        id: Uuid,
        parent: BlockParent,
    },
    SetBlockName {
        id: Uuid,
        name: String,
    },
    ListReferences {
        list: BlockReferenceList,
        completed: oneshot::Sender<Vec<BlockReference>>,
    },
    WatchReferences {
        list: BlockReferenceList,
    },
    CacheReferences {
        list: BlockReferenceList,
    },
    UnwatchReferences,
    ListBlockAccess {
        id: Uuid,
        completed: oneshot::Sender<Result<Vec<BlockAccessEntry>, String>>,
    },
    SetBlockAccess {
        id: Uuid,
    },
}

impl PendingRequest {
    fn debug_snapshot(&self, request_id: Uuid) -> PendingRequestDebugSnapshot {
        let (kind, details) = match self {
            Self::Create { id } => ("Create block", format!("block {id}")),
            Self::Read { id } => ("Read block", format!("block {id}")),
            Self::Update { id, operation_id } => (
                "Update block",
                format!("block {id}, operation {operation_id}"),
            ),
            Self::Batch { operations } => (
                "Update batch",
                format_operation_ids(operations.iter().copied()),
            ),
            Self::SetBlockParent { id, parent } => {
                ("Set block parent", format!("block {id}, parent {parent:?}"))
            }
            Self::SetBlockName { id, name } => {
                ("Set block name", format!("block {id}, name {name:?}"))
            }
            Self::ListReferences { list, .. } => ("List references", format!("list {list:?}")),
            Self::WatchReferences { list } => ("Watch references", format!("list {list:?}")),
            Self::CacheReferences { list } => ("Cache references", format!("list {list:?}")),
            Self::UnwatchReferences => ("Unwatch references", String::new()),
            Self::ListBlockAccess { id, .. } => ("List block access", format!("block {id}")),
            Self::SetBlockAccess { id } => ("Set block access", format!("block {id}")),
        };
        PendingRequestDebugSnapshot {
            request_id,
            kind: kind.into(),
            details,
        }
    }

    fn changes_data(&self) -> bool {
        matches!(
            self,
            Self::Create { .. }
                | Self::Update { .. }
                | Self::Batch { .. }
                | Self::SetBlockParent { .. }
                | Self::SetBlockName { .. }
                | Self::SetBlockAccess { .. }
        )
    }
}

enum DeferredRequest {
    SetBlockParent {
        id: Uuid,
        parent: BlockParent,
    },
    SetBlockName {
        id: Uuid,
        name: String,
    },
    ListReferences {
        list: BlockReferenceList,
        completed: oneshot::Sender<Vec<BlockReference>>,
    },
    WatchReferences {
        list: BlockReferenceList,
    },
    CacheReferences {
        list: BlockReferenceList,
    },
    UnwatchReferences {
        list: BlockReferenceList,
    },
    ListBlockAccess {
        id: Uuid,
        completed: oneshot::Sender<Result<Vec<BlockAccessEntry>, String>>,
    },
    SetBlockAccess {
        id: Uuid,
        account_id: Uuid,
        access: BlockAccess,
    },
}

impl DeferredRequest {
    fn debug_entry(&self) -> ClientDebugEntry {
        let (kind, details) = match self {
            Self::SetBlockParent { id, parent } => {
                ("Set block parent", format!("block {id}, parent {parent:?}"))
            }
            Self::SetBlockName { id, name } => {
                ("Set block name", format!("block {id}, name {name:?}"))
            }
            Self::ListReferences { list, .. } => ("List references", format!("list {list:?}")),
            Self::WatchReferences { list } => ("Watch references", format!("list {list:?}")),
            Self::CacheReferences { list } => ("Cache references", format!("list {list:?}")),
            Self::UnwatchReferences { list } => ("Unwatch references", format!("list {list:?}")),
            Self::ListBlockAccess { id, .. } => ("List block access", format!("block {id}")),
            Self::SetBlockAccess {
                id,
                account_id,
                access,
            } => (
                "Set block access",
                format!("block {id}, account {account_id}, access {access:?}"),
            ),
        };
        ClientDebugEntry {
            kind: kind.into(),
            details,
        }
    }

    fn changes_data(&self) -> bool {
        matches!(
            self,
            Self::SetBlockParent { .. } | Self::SetBlockName { .. } | Self::SetBlockAccess { .. }
        )
    }
}

fn client_message_changes_data(message: &ClientMessage) -> bool {
    matches!(
        message,
        ClientMessage::CreateBlock { .. }
            | ClientMessage::UpdateBlock { .. }
            | ClientMessage::UpdateBatch { .. }
            | ClientMessage::SetBlockParent { .. }
            | ClientMessage::SetBlockName { .. }
            | ClientMessage::SetBlockAccess { .. }
    )
}

fn client_message_debug_entry(message: &ClientMessage) -> ClientDebugEntry {
    let (kind, details) = match message {
        ClientMessage::CreateBlock {
            request_id,
            id,
            block_type,
            watch,
            ..
        } => (
            "Create block",
            format!("request {request_id}, block {id}, type {block_type}, watch {watch}"),
        ),
        ClientMessage::UpdateBlock {
            request_id,
            id,
            seq,
            operation_id,
            ..
        } => (
            "Update block",
            format!("request {request_id}, block {id}, seq {seq:?}, operation {operation_id}"),
        ),
        ClientMessage::UpdateBatch {
            request_id,
            updates,
        } => (
            "Update batch",
            format!(
                "request {request_id}, {}",
                format_operation_ids(
                    updates
                        .iter()
                        .map(|update| (update.id, update.operation_id))
                )
            ),
        ),
        ClientMessage::ReadBlock {
            request_id,
            id,
            watch,
        } => (
            "Read block",
            format!("request {request_id}, block {id}, watch {watch}"),
        ),
        ClientMessage::UnwatchBlock { request_id, id } => {
            ("Unwatch block", format!("request {request_id}, block {id}"))
        }
        ClientMessage::PostPresence { request_id, id, .. } => {
            ("Post presence", format!("request {request_id}, block {id}"))
        }
        ClientMessage::SetBlockParent {
            request_id,
            id,
            parent,
        } => (
            "Set block parent",
            format!("request {request_id}, block {id}, parent {parent:?}"),
        ),
        ClientMessage::SetBlockName {
            request_id,
            id,
            name,
        } => (
            "Set block name",
            format!("request {request_id}, block {id}, name {name:?}"),
        ),
        ClientMessage::ListReferences {
            request_id,
            list,
            watch,
        } => (
            "List references",
            format!("request {request_id}, list {list:?}, watch {watch}"),
        ),
        ClientMessage::UnwatchReferences { request_id, list } => (
            "Unwatch references",
            format!("request {request_id}, list {list:?}"),
        ),
        ClientMessage::ListBlockAccess { request_id, id } => (
            "List block access",
            format!("request {request_id}, block {id}"),
        ),
        ClientMessage::SetBlockAccess {
            request_id,
            id,
            account_id,
            access,
        } => (
            "Set block access",
            format!("request {request_id}, block {id}, account {account_id}, access {access:?}"),
        ),
    };
    ClientDebugEntry {
        kind: kind.into(),
        details,
    }
}

fn format_operation_ids(operations: impl Iterator<Item = (Uuid, Uuid)>) -> String {
    let operations: Vec<_> = operations
        .map(|(id, operation_id)| format!("{id}/{operation_id}"))
        .collect();
    format!(
        "{} operation{} [{}]",
        operations.len(),
        if operations.len() == 1 { "" } else { "s" },
        operations.join(", ")
    )
}

struct OutboundUpdate {
    seq: Option<u64>,
    operation_id: Uuid,
    operation: Vec<u8>,
    implicit_name: String,
    references: ReferenceDelta,
}

trait ErasedBlock: Send + Sync {
    fn id(&self) -> Uuid;
    fn block_type_id(&self) -> Uuid;
    fn author(&self) -> Option<Uuid>;
    fn dynamic_artifact(&self) -> Option<DynamicArtifactDescriptor>;
    fn debug_snapshot(&self) -> BlockDebugSnapshot;
    fn initial_data(&self) -> Option<Vec<u8>>;
    fn initial_name(&self) -> String;
    fn name(&self) -> String;
    fn initial_references(&self) -> Vec<Uuid>;
    fn created(&self);
    fn resolve_authored(
        &self,
        author: Uuid,
        snapshot: Vec<u8>,
        snapshot_seq: u64,
        operations: Vec<OperationRecord>,
        parent: BlockParent,
        name: String,
    );
    fn set_name(&self, name: String);
    fn set_parent(&self, parent: BlockParent);
    fn next_update(&self) -> Option<OutboundUpdate>;
    fn acknowledge(&self, operation_id: Uuid, seq: u64);
    fn sequence_conflict(&self, operation_id: Uuid, expected_seq: u64) -> bool;
    fn remote_operation(&self, operation: OperationRecord);
    fn is_synchronized(&self) -> bool;
    fn has_local_changes(&self) -> bool;
}

struct TypedBlock<B: Block> {
    id: Uuid,
    workspace_id: Uuid,
    author: RwLock<Option<Uuid>>,
    shared: Arc<BlockShared<B>>,
    state: RwLock<TypedState<B>>,
    loaded: watch::Sender<bool>,
    changed: watch::Sender<()>,
    relationships: RwLock<BlockRelationships>,
    name: RwLock<String>,
    dynamic_artifact: RwLock<Option<DynamicArtifactDescriptor>>,
    revision: AtomicU64,
    history: RwLock<HistoryStack<HistoryAction<B>>>,
}

type HistoryAction<B> = <<B as Block>::History as BlockHistory<B>>::Action;

#[derive(Clone, Copy)]
enum HistoryMode {
    None,
    Standalone,
    Grouped,
}

#[derive(Default)]
struct HistoryApplyResult {
    applied: bool,
    metadata: Option<HistoryMetadata>,
}

struct HistoryEntry<A> {
    action: A,
    bytes: usize,
    metadata: Option<HistoryMetadata>,
}

struct HistoryStack<A> {
    undo: VecDeque<HistoryEntry<A>>,
    redo: Vec<HistoryEntry<A>>,
    undo_bytes: usize,
    group_open: bool,
}

impl<A> Default for HistoryStack<A> {
    fn default() -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            undo_bytes: 0,
            group_open: false,
        }
    }
}

impl<A> HistoryStack<A> {
    fn push<B: Block<History = H>, H: BlockHistory<B, Action = A>>(
        &mut self,
        mut action: A,
        mode: HistoryMode,
        metadata: Option<HistoryMetadata>,
    ) {
        self.redo.clear();
        if matches!(mode, HistoryMode::Grouped) && self.group_open {
            if let Some(previous) = self.undo.back_mut() {
                let old_bytes = previous.bytes;
                match H::merge(&mut previous.action, action) {
                    Ok(()) => {
                        previous.bytes = H::action_bytes(&previous.action).saturating_add(
                            previous.metadata.as_ref().map_or(0, HistoryMetadata::bytes),
                        );
                        self.undo_bytes = self
                            .undo_bytes
                            .saturating_sub(old_bytes)
                            .saturating_add(previous.bytes);
                        self.evict();
                        return;
                    }
                    Err(next) => action = next,
                }
            }
        }
        let bytes = H::action_bytes(&action)
            .saturating_add(metadata.as_ref().map_or(0, HistoryMetadata::bytes));
        self.undo_bytes = self.undo_bytes.saturating_add(bytes);
        self.undo.push_back(HistoryEntry {
            action,
            bytes,
            metadata,
        });
        self.group_open = matches!(mode, HistoryMode::Grouped);
        self.evict();
    }

    fn evict(&mut self) {
        while self.undo.len() > MAX_HISTORY_ACTIONS || self.undo_bytes > MAX_HISTORY_BYTES {
            if self.undo.len() == 1 {
                break;
            }
            if let Some(entry) = self.undo.pop_front() {
                self.undo_bytes = self.undo_bytes.saturating_sub(entry.bytes);
            }
        }
    }

    fn finish_group(&mut self) {
        self.group_open = false;
    }

    fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn apply<R>(
        &mut self,
        direction: HistoryDirection,
        apply: impl FnOnce(&mut A) -> R,
    ) -> Option<(R, Option<HistoryMetadata>)> {
        self.group_open = false;
        match direction {
            HistoryDirection::Undo => {
                let mut entry = self.undo.pop_back()?;
                self.undo_bytes = self.undo_bytes.saturating_sub(entry.bytes);
                let result = apply(&mut entry.action);
                let metadata = entry.metadata.clone();
                self.redo.push(entry);
                Some((result, metadata))
            }
            HistoryDirection::Redo => {
                let mut entry = self.redo.pop()?;
                let result = apply(&mut entry.action);
                let metadata = entry.metadata.clone();
                self.undo_bytes = self.undo_bytes.saturating_add(entry.bytes);
                self.undo.push_back(entry);
                Some((result, metadata))
            }
        }
    }
}

struct TypedState<B: Block> {
    initial: Option<B>,
    confirmed: Option<B>,
    confirmed_seq: u64,
    acknowledged_seq: u64,
    pending: VecDeque<PendingOperation<B, B::Operation>>,
    in_flight: HashSet<Uuid>,
    buffered: BTreeMap<u64, OperationRecord>,
    ready: bool,
}

struct PendingOperation<B, O> {
    id: Uuid,
    operation: StoredOperation<B, O>,
    implicit_name: String,
    references: ReferenceDelta,
}

impl<B: Block> TypedBlock<B> {
    fn apply_stored_operation(value: &mut B, operation: &StoredOperation<B, B::Operation>) {
        match operation {
            StoredOperation::Operate(operation) => B::apply_operation(value, operation),
            StoredOperation::Replace(replacement) => value.clone_from(replacement),
        }
    }

    #[cfg(test)]
    fn created(id: Uuid, shared: Arc<BlockShared<B>>, initial: B) -> Self {
        Self::created_by(id, Uuid::nil(), Uuid::nil(), shared, initial, None)
    }

    fn created_by(
        id: Uuid,
        author: Uuid,
        workspace_id: Uuid,
        shared: Arc<BlockShared<B>>,
        initial: B,
        dynamic_artifact: Option<DynamicArtifactDescriptor>,
    ) -> Self {
        let references = normalized_references(initial.references_for_workspace(workspace_id));
        let name = initial.implicit_name();
        Self {
            id,
            workspace_id,
            author: RwLock::new(Some(author)),
            shared,
            state: RwLock::new(TypedState {
                initial: Some(initial.clone()),
                confirmed: Some(initial),
                confirmed_seq: 0,
                acknowledged_seq: 0,
                pending: VecDeque::new(),
                in_flight: HashSet::new(),
                buffered: BTreeMap::new(),
                ready: false,
            }),
            loaded: watch::channel(false).0,
            changed: watch::channel(()).0,
            relationships: RwLock::new(BlockRelationships {
                parent: BlockParent::Orphaned,
                references,
                backrefs: Vec::new(),
            }),
            name: RwLock::new(name),
            dynamic_artifact: RwLock::new(dynamic_artifact),
            revision: AtomicU64::new(0),
            history: RwLock::new(HistoryStack::default()),
        }
    }

    fn unresolved(id: Uuid, workspace_id: Uuid, shared: Arc<BlockShared<B>>) -> Self {
        Self {
            id,
            workspace_id,
            author: RwLock::new(None),
            shared,
            state: RwLock::new(TypedState {
                initial: None,
                confirmed: None,
                confirmed_seq: 0,
                acknowledged_seq: 0,
                pending: VecDeque::new(),
                in_flight: HashSet::new(),
                buffered: BTreeMap::new(),
                ready: false,
            }),
            loaded: watch::channel(false).0,
            changed: watch::channel(()).0,
            relationships: RwLock::new(BlockRelationships {
                parent: BlockParent::Orphaned,
                references: Vec::new(),
                backrefs: Vec::new(),
            }),
            name: RwLock::new(String::new()),
            dynamic_artifact: RwLock::new(None),
            revision: AtomicU64::new(0),
            history: RwLock::new(HistoryStack::default()),
        }
    }

    fn rebuild_visible(&self, state: &TypedState<B>) {
        let Some(mut value) = state.confirmed.clone() else {
            return;
        };
        for pending in &state.pending {
            Self::apply_stored_operation(&mut value, &pending.operation);
        }
        *self.shared.value.write() = Some(value);
        self.changed.send_replace(());
    }

    fn local_operations(
        &self,
        operations: Vec<B::Operation>,
        history_mode: HistoryMode,
        history_metadata: Option<HistoryMetadata>,
    ) {
        let mut state = self.state.write();
        let record_history = B::History::ENABLED && !matches!(history_mode, HistoryMode::None);
        let history_action = {
            let mut visible = self.shared.value.write();
            let value = visible
                .as_mut()
                .unwrap_or_else(|| fatal("cannot operate on an unresolved block"));
            let history_before = record_history.then(|| B::History::snapshot(value));
            for operation in &operations {
                let before =
                    normalized_references(value.references_for_workspace(self.workspace_id));
                B::apply_operation(value, operation);
                let implicit_name = value.implicit_name();
                let after =
                    normalized_references(value.references_for_workspace(self.workspace_id));
                self.relationships.write().references.clone_from(&after);
                state.pending.push_back(PendingOperation {
                    id: Uuid::new_v4(),
                    operation: StoredOperation::Operate(operation.clone()),
                    references: reference_delta(&before, &after),
                    implicit_name,
                });
            }
            self.revision.fetch_add(1, Ordering::Relaxed);
            self.changed.send_replace(());
            history_before.and_then(|before| B::History::action(before, value, &operations))
        };
        drop(state);
        if !matches!(history_mode, HistoryMode::None) {
            if let Some(action) = history_action {
                self.history
                    .write()
                    .push::<B, B::History>(action, history_mode, history_metadata);
            }
        }
    }

    fn local_crdt_edit<R>(
        &self,
        history_mode: HistoryMode,
        history_metadata: Option<HistoryMetadata>,
        edit: impl FnOnce(&mut CrdtOperationTransaction<'_, B>) -> R,
    ) -> (R, bool) {
        assert!(B::CRDT, "in-place editing requires a CRDT block");
        let mut state = self.state.write();
        let record_history = B::History::ENABLED && !matches!(history_mode, HistoryMode::None);
        let (result, applied, history_action) = {
            let mut visible = self.shared.value.write();
            let value = visible
                .as_mut()
                .unwrap_or_else(|| fatal("cannot operate on an unresolved block"));
            let history_before = record_history.then(|| B::History::snapshot(value));
            let mut transaction = CrdtOperationTransaction {
                current: value,
                workspace_id: self.workspace_id,
                applied: Vec::new(),
            };
            let result = edit(&mut transaction);
            let CrdtOperationTransaction {
                current: _,
                workspace_id: _,
                applied,
            } = transaction;
            let operations = applied
                .iter()
                .map(|applied| applied.operation.clone())
                .collect::<Vec<_>>();
            let history_action =
                history_before.and_then(|before| B::History::action(before, value, &operations));
            (result, applied, history_action)
        };
        if applied.is_empty() {
            return (result, false);
        }
        for applied in applied {
            let after = normalized_references(
                self.shared
                    .value
                    .read()
                    .as_ref()
                    .map_or_else(Vec::new, |value| {
                        value.references_for_workspace(self.workspace_id)
                    }),
            );
            self.relationships.write().references = after;
            state.pending.push_back(PendingOperation {
                id: Uuid::new_v4(),
                operation: StoredOperation::Operate(applied.operation),
                references: applied.references,
                implicit_name: applied.implicit_name,
            });
        }
        self.revision.fetch_add(1, Ordering::Relaxed);
        self.changed.send_replace(());
        drop(state);
        if let Some(action) = history_action {
            self.history
                .write()
                .push::<B, B::History>(action, history_mode, history_metadata);
        }
        (result, true)
    }

    fn local_replace(&self, replacement: B) {
        let mut state = self.state.write();
        let mut visible = self.shared.value.write();
        let value = visible
            .as_mut()
            .unwrap_or_else(|| fatal("cannot replace an unresolved block"));
        let before = normalized_references(value.references_for_workspace(self.workspace_id));
        value.clone_from(&replacement);
        let after = normalized_references(value.references_for_workspace(self.workspace_id));
        self.relationships.write().references.clone_from(&after);
        state.pending.push_back(PendingOperation {
            id: Uuid::new_v4(),
            operation: StoredOperation::Replace(replacement),
            implicit_name: value.implicit_name(),
            references: reference_delta(&before, &after),
        });
        self.history.write().finish_group();
        self.revision.fetch_add(1, Ordering::Relaxed);
        self.changed.send_replace(());
    }

    #[cfg(test)]
    fn local_operation(&self, operation: B::Operation) {
        self.local_operations(vec![operation], HistoryMode::Standalone, None);
    }

    fn apply_history(&self, direction: HistoryDirection) -> HistoryApplyResult {
        if B::CRDT {
            return self.apply_crdt_history(direction);
        }
        let Some(current) = self.shared.value.read().clone() else {
            return HistoryApplyResult::default();
        };
        let Some((operations, metadata)) = self.history.write().apply(direction, |action| {
            B::History::operations(&current, action, direction)
        }) else {
            return HistoryApplyResult::default();
        };
        if operations.is_empty() {
            return HistoryApplyResult::default();
        }
        self.local_operations(operations, HistoryMode::None, None);
        HistoryApplyResult {
            applied: true,
            metadata,
        }
    }

    fn apply_crdt_history(&self, direction: HistoryDirection) -> HistoryApplyResult {
        let mut state = self.state.write();
        let (applied, metadata) = {
            let mut visible = self.shared.value.write();
            let value = visible
                .as_mut()
                .unwrap_or_else(|| fatal("cannot operate on an unresolved block"));
            let mut transaction = CrdtOperationTransaction {
                current: value,
                workspace_id: self.workspace_id,
                applied: Vec::new(),
            };
            let Some(((), metadata)) = self.history.write().apply(direction, |action| {
                B::History::apply_operations(&mut transaction, action, direction);
            }) else {
                return HistoryApplyResult::default();
            };
            (transaction.applied, metadata)
        };
        if applied.is_empty() {
            return HistoryApplyResult::default();
        }
        for applied in applied {
            self.relationships.write().references = normalized_references(
                self.shared
                    .value
                    .read()
                    .as_ref()
                    .map_or_else(Vec::new, |value| {
                        value.references_for_workspace(self.workspace_id)
                    }),
            );
            state.pending.push_back(PendingOperation {
                id: Uuid::new_v4(),
                operation: StoredOperation::Operate(applied.operation),
                implicit_name: applied.implicit_name,
                references: applied.references,
            });
        }
        self.revision.fetch_add(1, Ordering::Relaxed);
        self.changed.send_replace(());
        HistoryApplyResult {
            applied: true,
            metadata,
        }
    }

    #[cfg(test)]
    fn resolve(
        &self,
        snapshot: Vec<u8>,
        snapshot_seq: u64,
        operations: Vec<OperationRecord>,
        parent: BlockParent,
        name: String,
    ) {
        <Self as ErasedBlock>::resolve_authored(
            self,
            Uuid::nil(),
            snapshot,
            snapshot_seq,
            operations,
            parent,
            name,
        );
    }
}

impl<B: Block> ErasedBlock for TypedBlock<B> {
    fn id(&self) -> Uuid {
        self.id
    }

    fn block_type_id(&self) -> Uuid {
        B::TYPE_ID
    }

    fn author(&self) -> Option<Uuid> {
        *self.author.read()
    }

    fn dynamic_artifact(&self) -> Option<DynamicArtifactDescriptor> {
        self.dynamic_artifact.read().clone()
    }

    fn debug_snapshot(&self) -> BlockDebugSnapshot {
        let state = self.state.read();
        let synchronized = state.ready
            && state.pending.is_empty()
            && state.in_flight.is_empty()
            && state.buffered.is_empty()
            && (!B::CRDT || state.confirmed_seq >= state.acknowledged_seq);
        let has_local_changes = (state.initial.is_some() && !state.ready)
            || !state.pending.is_empty()
            || !state.in_flight.is_empty();
        BlockDebugSnapshot {
            id: self.id,
            block_type: B::TYPE_ID,
            name: self.name.read().clone(),
            crdt: B::CRDT,
            ready: state.ready,
            synchronized,
            has_local_changes,
            confirmed_seq: state.confirmed_seq,
            acknowledged_seq: state.acknowledged_seq,
            pending_operations: state.pending.len(),
            in_flight_operations: state.in_flight.len(),
            buffered_operations: state.buffered.len(),
        }
    }

    fn initial_data(&self) -> Option<Vec<u8>> {
        self.state.read().initial.as_ref().map(|initial| {
            serde_json::to_vec(&StoredBlock {
                value: initial,
                dynamic_artifact: self.dynamic_artifact.read().clone(),
            })
            .unwrap_or_else(|error| fatal(format!("failed to serialize block: {error}")))
        })
    }

    fn initial_name(&self) -> String {
        self.state
            .read()
            .initial
            .as_ref()
            .map_or_else(String::new, Block::implicit_name)
    }

    fn name(&self) -> String {
        self.name.read().clone()
    }

    fn initial_references(&self) -> Vec<Uuid> {
        self.state
            .read()
            .initial
            .as_ref()
            .map_or_else(Vec::new, |initial| {
                normalized_references(initial.references_for_workspace(self.workspace_id))
            })
    }

    fn created(&self) {
        let mut state = self.state.write();
        state.ready = true;
        if B::CRDT {
            state.confirmed = None;
        }
        drop(state);
        self.loaded.send_replace(true);
    }

    fn resolve_authored(
        &self,
        author: Uuid,
        snapshot: Vec<u8>,
        snapshot_seq: u64,
        operations: Vec<OperationRecord>,
        parent: BlockParent,
        name: String,
    ) {
        *self.author.write() = Some(author);
        let stored: StoredBlock<B> = serde_json::from_slice(&snapshot).unwrap_or_else(|error| {
            fatal(format!("failed to deserialize block snapshot: {error}"))
        });
        let mut value = stored.value;
        *self.dynamic_artifact.write() = stored.dynamic_artifact;
        let mut seq = snapshot_seq;
        for record in operations {
            if record.seq != seq + 1 {
                fatal("server returned noncontiguous block history");
            }
            let operation: StoredOperation<B, B::Operation> =
                serde_json::from_slice(&record.operation).unwrap_or_else(|error| {
                    fatal(format!("failed to deserialize operation: {error}"))
                });
            Self::apply_stored_operation(&mut value, &operation);
            seq = record.seq;
        }
        self.revision.store(seq, Ordering::Relaxed);
        let references = normalized_references(value.references_for_workspace(self.workspace_id));
        let mut state = self.state.write();
        {
            let mut relationships = self.relationships.write();
            relationships.parent = parent;
            relationships.references = references;
        }
        *self.name.write() = name;
        if B::CRDT {
            for pending in &state.pending {
                Self::apply_stored_operation(&mut value, &pending.operation);
            }
            *self.shared.value.write() = Some(value);
            self.changed.send_replace(());
            state.confirmed = None;
        } else {
            state.confirmed = Some(value);
            self.recompute_pending_references(&mut state);
            self.rebuild_visible(&state);
        }
        state.confirmed_seq = seq;
        state.ready = true;
        self.loaded.send_replace(true);
    }

    fn set_parent(&self, parent: BlockParent) {
        self.relationships.write().parent = parent;
    }

    fn set_name(&self, name: String) {
        *self.name.write() = name;
        self.changed.send_replace(());
    }

    fn next_update(&self) -> Option<OutboundUpdate> {
        let mut state = self.state.write();
        if !state.ready || (!B::CRDT && !state.in_flight.is_empty()) {
            return None;
        }
        let pending = state
            .pending
            .iter()
            .find(|pending| !state.in_flight.contains(&pending.id))?;
        let pending_id = pending.id;
        let update = OutboundUpdate {
            seq: (!B::CRDT).then_some(state.confirmed_seq + 1),
            operation_id: pending_id,
            operation: serde_json::to_vec(&pending.operation)
                .unwrap_or_else(|error| fatal(format!("failed to serialize operation: {error}"))),
            implicit_name: pending.implicit_name.clone(),
            references: pending.references.clone(),
        };
        state.in_flight.insert(pending_id);
        Some(update)
    }

    fn acknowledge(&self, operation_id: Uuid, seq: u64) {
        let mut state = self.state.write();
        if B::CRDT {
            let Some(index) = state
                .pending
                .iter()
                .position(|pending| pending.id == operation_id)
            else {
                state.in_flight.remove(&operation_id);
                state.acknowledged_seq = state.acknowledged_seq.max(seq);
                return;
            };
            state.pending.remove(index);
            state.in_flight.remove(&operation_id);
            state.acknowledged_seq = state.acknowledged_seq.max(seq);
            return;
        }
        if seq <= state.confirmed_seq {
            return;
        }
        let Some(front) = state.pending.front() else {
            fatal("acknowledged operation is not pending");
        };
        if front.id != operation_id {
            fatal("acknowledged operation is not at the front of the pending queue");
        }
        if seq != state.confirmed_seq + 1 {
            fatal("update acknowledgement is not contiguous");
        }
        let operation = state.pending.pop_front().unwrap().operation;
        Self::apply_stored_operation(state.confirmed.as_mut().unwrap(), &operation);
        state.confirmed_seq = seq;
        state.in_flight.remove(&operation_id);
        self.rebuild_visible(&state);
    }

    fn sequence_conflict(&self, operation_id: Uuid, expected_seq: u64) -> bool {
        if B::CRDT {
            fatal("CRDT update received a sequence conflict");
        }
        let mut state = self.state.write();
        if !state.in_flight.remove(&operation_id) {
            fatal("sequence conflict referenced the wrong operation");
        }
        state.confirmed_seq + 1 >= expected_seq
    }

    fn remote_operation(&self, record: OperationRecord) {
        let mut state = self.state.write();
        if record.seq <= state.confirmed_seq {
            return;
        }
        state.buffered.entry(record.seq).or_insert(record);
        loop {
            let next_seq = state.confirmed_seq + 1;
            let Some(record) = state.buffered.remove(&next_seq) else {
                break;
            };
            self.apply_remote_operation(&mut state, record);
        }
    }

    fn is_synchronized(&self) -> bool {
        let state = self.state.read();
        state.ready
            && state.pending.is_empty()
            && state.in_flight.is_empty()
            && state.buffered.is_empty()
            && (!B::CRDT || state.confirmed_seq >= state.acknowledged_seq)
    }

    fn has_local_changes(&self) -> bool {
        let state = self.state.read();
        (state.initial.is_some() && !state.ready)
            || !state.pending.is_empty()
            || !state.in_flight.is_empty()
    }
}

impl<B: Block> TypedBlock<B> {
    fn recompute_pending_references(&self, state: &mut TypedState<B>) {
        let Some(mut value) = state.confirmed.clone() else {
            return;
        };
        for pending in &mut state.pending {
            let before = normalized_references(value.references_for_workspace(self.workspace_id));
            Self::apply_stored_operation(&mut value, &pending.operation);
            let after = normalized_references(value.references_for_workspace(self.workspace_id));
            pending.references = reference_delta(&before, &after);
        }
    }

    fn apply_remote_operation(&self, state: &mut TypedState<B>, record: OperationRecord) {
        if B::CRDT {
            let remote: StoredOperation<B, B::Operation> =
                serde_json::from_slice(&record.operation).unwrap_or_else(|error| {
                    fatal(format!("failed to deserialize remote operation: {error}"))
                });
            if let Some(value) = self.shared.value.write().as_mut() {
                Self::apply_stored_operation(value, &remote);
                self.changed.send_replace(());
            }
            if let Some(index) = state
                .pending
                .iter()
                .position(|pending| pending.id == record.operation_id)
            {
                state.pending.remove(index);
            }
            state.in_flight.remove(&record.operation_id);
            state.confirmed_seq = record.seq;
            self.revision.fetch_add(1, Ordering::Relaxed);
            return;
        }

        if state
            .pending
            .front()
            .is_some_and(|pending| pending.id == record.operation_id)
        {
            let pending = state.pending.pop_front().unwrap();
            Self::apply_stored_operation(state.confirmed.as_mut().unwrap(), &pending.operation);
            state.confirmed_seq = record.seq;
            state.in_flight.remove(&record.operation_id);
            self.revision.fetch_add(1, Ordering::Relaxed);
            self.rebuild_visible(state);
            return;
        }

        let remote: StoredOperation<B, B::Operation> = serde_json::from_slice(&record.operation)
            .unwrap_or_else(|error| {
                fatal(format!("failed to deserialize remote operation: {error}"))
            });
        Self::apply_stored_operation(state.confirmed.as_mut().unwrap(), &remote);
        state.confirmed_seq = record.seq;
        if let StoredOperation::Operate(remote) = &remote {
            for pending in &mut state.pending {
                if let StoredOperation::Operate(local) = &mut pending.operation {
                    B::transform_operation(local, remote);
                }
            }
        }
        self.revision.fetch_add(1, Ordering::Relaxed);
        self.recompute_pending_references(state);
        self.rebuild_visible(state);
    }
}

fn normalized_references(mut references: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    references.retain(|reference| seen.insert(*reference));
    references
}

fn reference_delta(before: &[Uuid], after: &[Uuid]) -> ReferenceDelta {
    ReferenceDelta {
        before: before.to_vec(),
        after: after.to_vec(),
    }
}

fn fatal(message: impl AsRef<str>) -> ! {
    let message = message.as_ref();
    #[cfg(target_arch = "wasm32")]
    web_sys::console::error_1(&format!("fatal block client error: {message}").into());
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("fatal block client error: {message}");
    process::abort()
}
