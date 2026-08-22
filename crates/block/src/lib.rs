use std::collections::BTreeMap;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Account {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRole {
    /// Has full access to every block in the workspace, regardless of the
    /// per-block permissions recorded for it.
    Administrator,
    /// Only reaches the blocks it authored or was explicitly granted access to.
    Editor,
}

impl WorkspaceRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Administrator => "Administrator",
            Self::Editor => "Editor",
        }
    }
}

/// How much of a block an account may reach. The variants are ordered from
/// least to most access so effective permissions can be combined with `max`.
#[derive(Clone, Copy, Debug, Deserialize, Ord, PartialOrd, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockAccess {
    /// The block is inaccessible and is filtered out of listings.
    None,
    /// The block appears in listings but cannot be opened.
    KnowExists,
    /// The block can be read but not changed.
    View,
    /// The block can be read and changed.
    Edit,
}

impl BlockAccess {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "No access",
            Self::KnowExists => "Knows it exists",
            Self::View => "Can view",
            Self::Edit => "Can edit",
        }
    }

    pub fn can_know_exists(self) -> bool {
        self >= Self::KnowExists
    }

    pub fn can_view(self) -> bool {
        self >= Self::View
    }

    pub fn can_edit(self) -> bool {
        self == Self::Edit
    }
}

/// One workspace member's access to a single block, as reported to a client
/// that is managing sharing for that block.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BlockAccessEntry {
    pub account: Account,
    pub role: WorkspaceRole,
    /// The permission recorded directly against this block, if any.
    pub granted: Option<BlockAccess>,
    /// The permission the account actually has, after inheritance.
    pub effective: BlockAccess,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub owner_id: Uuid,
    pub role: WorkspaceRole,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct WorkspaceInvitation {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub workspace_name: String,
    pub email: String,
    pub role: WorkspaceRole,
    pub invited_by: Uuid,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagementErrorCode {
    AccountAlreadyMember,
    AccountNotFound,
    EmailAlreadyRegistered,
    InvalidCredentials,
    InvalidEmail,
    InvalidMessage,
    InvalidName,
    InvalidPassword,
    InvalidToken,
    InvitationAlreadyExists,
    InvitationNotFound,
    PermissionDenied,
    RegistrationDisabled,
    StorageError,
    UnsupportedMessage,
    WorkspaceNotFound,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum ManagementClientMessage {
    Register {
        request_id: Uuid,
        email: String,
        display_name: String,
        password: String,
    },
    Login {
        request_id: Uuid,
        email: String,
        password: String,
    },
    Logout {
        request_id: Uuid,
        token: String,
    },
    ListWorkspaces {
        request_id: Uuid,
        token: String,
    },
    CreateWorkspace {
        request_id: Uuid,
        token: String,
        name: String,
    },
    ListInvitations {
        request_id: Uuid,
        token: String,
    },
    Invite {
        request_id: Uuid,
        token: String,
        workspace_id: Uuid,
        email: String,
        role: WorkspaceRole,
    },
    RespondInvitation {
        request_id: Uuid,
        token: String,
        invitation_id: Uuid,
        accept: bool,
    },
}

impl ManagementClientMessage {
    pub fn request_id(&self) -> Uuid {
        match self {
            Self::Register { request_id, .. }
            | Self::Login { request_id, .. }
            | Self::Logout { request_id, .. }
            | Self::ListWorkspaces { request_id, .. }
            | Self::CreateWorkspace { request_id, .. }
            | Self::ListInvitations { request_id, .. }
            | Self::Invite { request_id, .. }
            | Self::RespondInvitation { request_id, .. } => *request_id,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ManagementServerMessage {
    Account {
        request_id: Uuid,
        account: Account,
        token: String,
    },
    Workspace {
        request_id: Uuid,
        workspace: Workspace,
    },
    Workspaces {
        request_id: Uuid,
        workspaces: Vec<Workspace>,
    },
    Invitation {
        request_id: Uuid,
        invitation: WorkspaceInvitation,
    },
    Invitations {
        request_id: Uuid,
        invitations: Vec<WorkspaceInvitation>,
    },
    Ok {
        request_id: Uuid,
    },
    Error {
        request_id: Option<Uuid>,
        code: ManagementErrorCode,
        message: String,
    },
}

pub trait Block: Clone + Serialize + DeserializeOwned + Send + Sync + 'static {
    type Operation: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;
    type History: BlockHistory<Self>;

    const TYPE_ID: Uuid;
    const CRDT: bool = false;

    fn apply_operation(block: &mut Self, operation: &Self::Operation);

    /// Like `apply_operation`, but for block types whose state needs to know
    /// who is responsible for each operation. `author` is the operation's
    /// server-verified account id (`OperationRecord::author`), never a value
    /// that the operation's own bytes could claim for themselves. Defaults to
    /// ignoring it.
    fn apply_authored_operation(block: &mut Self, operation: &Self::Operation, author: Uuid) {
        let _ = author;
        Self::apply_operation(block, operation);
    }

    /// A name this block type can derive from its own content, or `None` if
    /// it has nothing more useful to say than its type. The result becomes
    /// the block's `name` property unless a client has manually renamed it.
    fn implicit_name(&self) -> Option<String> {
        None
    }

    fn transform_operation(_local: &mut Self::Operation, _remote: &Self::Operation) {}

    fn references(&self) -> Vec<Uuid> {
        Vec::new()
    }

    fn references_for_workspace(&self, _workspace_id: Uuid) -> Vec<Uuid> {
        self.references()
    }

    /// Operations that add `block_id` as a child, or `None` if this block
    /// type does not support child blocks.
    fn add_child(&self, _block_id: Uuid) -> Option<Vec<Self::Operation>> {
        None
    }

    /// Operations that remove `block_id` as a child, or `None` if this block
    /// type does not support child blocks.
    fn delete_child(&self, _block_id: Uuid) -> Option<Vec<Self::Operation>> {
        None
    }

    /// Operations that replace child `old` with `new`, or `None` if this
    /// block type does not support child blocks.
    fn replace_child(&self, _old: Uuid, _new: Uuid) -> Option<Vec<Self::Operation>> {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryDirection {
    Undo,
    Redo,
}

pub trait BlockHistory<B: Block>: Send + Sync + 'static {
    type Action: Send + Sync + 'static;
    type Snapshot;

    const ENABLED: bool = true;

    fn snapshot(block: &B) -> Self::Snapshot;

    fn action(
        before: Self::Snapshot,
        after: &B,
        operations: &[B::Operation],
    ) -> Option<Self::Action>;

    fn action_bytes(action: &Self::Action) -> usize;

    fn merge(_previous: &mut Self::Action, next: Self::Action) -> Result<(), Self::Action> {
        Err(next)
    }

    fn operations(
        _current: &B,
        _action: &mut Self::Action,
        _direction: HistoryDirection,
    ) -> Vec<B::Operation> {
        Vec::new()
    }

    fn apply_operations<T: BlockHistoryTransaction<B>>(
        transaction: &mut T,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) {
        for operation in Self::operations(transaction.current(), action, direction) {
            transaction.apply(operation);
        }
    }
}

pub trait BlockHistoryTransaction<B: Block> {
    fn current(&self) -> &B;

    fn apply(&mut self, operation: B::Operation);
}

pub struct NoHistory;

impl<B: Block> BlockHistory<B> for NoHistory {
    type Action = ();
    type Snapshot = B;

    const ENABLED: bool = false;

    fn snapshot(block: &B) -> Self::Snapshot {
        block.clone()
    }

    fn action(_before: B, _after: &B, _operations: &[B::Operation]) -> Option<Self::Action> {
        None
    }

    fn action_bytes(_action: &Self::Action) -> usize {
        0
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct OperationRecord {
    pub seq: u64,
    pub operation_id: Uuid,
    pub author: Uuid,
    pub operation: Vec<u8>,
    pub references: ReferenceDelta,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct ReferenceDelta {
    pub before: Vec<Uuid>,
    pub after: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BlockOperation {
    pub id: Uuid,
    pub properties: BTreeMap<Uuid, Vec<u8>>,
    pub operation: OperationRecord,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BlockUpdate {
    pub id: Uuid,
    pub properties: BTreeMap<Uuid, Vec<u8>>,
    /// Whether the block is generated from another one after this update.
    pub dynamic_artifact: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    pub operation_id: Uuid,
    pub operation: Vec<u8>,
    pub references: ReferenceDelta,
}

#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockParent {
    Orphaned,
    Root,
    Uuid(Uuid),
}

#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockReferenceList {
    Roots,
    Orphans,
    Parents(Uuid),
    References(Uuid),
    Backrefs(Uuid),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BlockReference {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub block_type: Uuid,
    pub author: Uuid,
    /// Every property key and value recorded against the block. Properties
    /// are opaque to the server; only the client interprets them (e.g. the
    /// well-known "name" property).
    pub properties: BTreeMap<Uuid, Vec<u8>>,
    pub parent: BlockParent,
    pub references: usize,
    /// Whether the block is generated from another one, so a client can mark
    /// it where it is listed without opening it.
    pub dynamic_artifact: bool,
    /// What the listing account may do with the block, so a client can tell
    /// which of the things it offers would be refused without opening it.
    pub access: BlockAccess,
}

/// A generic guard against abusively large property values. The server does
/// not interpret properties, so this is the only limit it enforces.
pub const MAX_PROPERTY_VALUE_BYTES: usize = 4096;

/// Identifies one live connection, not an account: the same account may have
/// several clients connected at once (e.g. multiple tabs or devices) editing
/// the same block. Assigned by the server when a connection is accepted and
/// not stable across reconnects.
pub type ClientId = u64;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    CreateBlock,
    UpdateBlock,
    UpdateBatch,
    ReadBlock,
    UnwatchBlock,
    SetPresence,
    SetBlockParent,
    ListReferences,
    UnwatchReferences,
    ListBlockAccess,
    SetBlockAccess,
    SetBlockProperty,
    CloseClient,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    AccountNotFound,
    BlockAlreadyExists,
    BlockNotFound,
    ConflictingOperationId,
    InvalidMessage,
    InvalidSeq,
    NotWatching,
    ParentCycle,
    PermissionDenied,
    StorageError,
    UnsupportedMessage,
}

/// One websocket frame from a client. A single connection carries any number of
/// separate clients: `client` names which of them a frame belongs to, and is
/// absent for the connection's own client. Each one gets its own watches,
/// presence and sequencing on the server, exactly as if it had dialled in on a
/// connection of its own.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ClientEnvelope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<Uuid>,
    #[serde(flatten)]
    pub message: ClientMessage,
}

/// One websocket frame from the server, addressed to the client of the
/// connection named by `client`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ServerEnvelope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<Uuid>,
    #[serde(flatten)]
    pub message: ServerMessage,
}

impl ClientEnvelope {
    pub fn new(client: Option<Uuid>, message: ClientMessage) -> Self {
        Self { client, message }
    }
}

impl ServerEnvelope {
    pub fn new(client: Option<Uuid>, message: ServerMessage) -> Self {
        Self { client, message }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum ClientMessage {
    CreateBlock {
        request_id: Uuid,
        id: Uuid,
        #[serde(rename = "type")]
        block_type: Uuid,
        data: Vec<u8>,
        properties: BTreeMap<Uuid, Vec<u8>>,
        dynamic_artifact: bool,
        references: Vec<Uuid>,
        watch: bool,
    },
    UpdateBlock {
        request_id: Uuid,
        id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
        operation_id: Uuid,
        operation: Vec<u8>,
        properties: BTreeMap<Uuid, Vec<u8>>,
        dynamic_artifact: bool,
        references: ReferenceDelta,
    },
    UpdateBatch {
        request_id: Uuid,
        updates: Vec<BlockUpdate>,
    },
    ReadBlock {
        request_id: Uuid,
        id: Uuid,
        watch: bool,
    },
    UnwatchBlock {
        request_id: Uuid,
        id: Uuid,
    },
    /// Sets or removes a presence value for a block the client is currently
    /// watching. The server keeps the latest value per `(client, presence_id)`
    /// and broadcasts changes to every other client watching the block.
    SetPresence {
        request_id: Uuid,
        id: Uuid,
        presence_id: Uuid,
        data: Option<Vec<u8>>,
    },
    SetBlockParent {
        request_id: Uuid,
        id: Uuid,
        parent: BlockParent,
    },
    SetBlockProperty {
        request_id: Uuid,
        id: Uuid,
        key: Uuid,
        value: Vec<u8>,
    },
    ListReferences {
        request_id: Uuid,
        list: BlockReferenceList,
        watch: bool,
    },
    UnwatchReferences {
        request_id: Uuid,
        list: BlockReferenceList,
    },
    ListBlockAccess {
        request_id: Uuid,
        id: Uuid,
    },
    SetBlockAccess {
        request_id: Uuid,
        id: Uuid,
        account_id: Uuid,
        access: BlockAccess,
    },
    /// Ends one of the connection's clients, releasing its watches and
    /// presence. The connection itself stays open for its other clients.
    CloseClient {
        request_id: Uuid,
    },
}

impl ClientMessage {
    pub fn request_id(&self) -> Uuid {
        match self {
            Self::CreateBlock { request_id, .. }
            | Self::UpdateBlock { request_id, .. }
            | Self::UpdateBatch { request_id, .. }
            | Self::ReadBlock { request_id, .. }
            | Self::UnwatchBlock { request_id, .. }
            | Self::SetPresence { request_id, .. }
            | Self::SetBlockParent { request_id, .. }
            | Self::ListReferences { request_id, .. }
            | Self::UnwatchReferences { request_id, .. }
            | Self::ListBlockAccess { request_id, .. }
            | Self::SetBlockAccess { request_id, .. }
            | Self::CloseClient { request_id }
            | Self::SetBlockProperty { request_id, .. } => *request_id,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ServerMessage {
    Ok {
        request_id: Uuid,
        command: CommandKind,
        id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        operation_id: Option<Uuid>,
    },
    #[serde(rename = "read_block")]
    ReadBlock {
        request_id: Uuid,
        command: CommandKind,
        id: Uuid,
        #[serde(rename = "type")]
        block_type: Uuid,
        author: Uuid,
        snapshot: Vec<u8>,
        snapshot_seq: u64,
        operations: Vec<OperationRecord>,
        parent: BlockParent,
        properties: BTreeMap<Uuid, Vec<u8>>,
        /// What the reading account may do with the block, so the client knows
        /// whether to let it be changed without asking the server first.
        access: BlockAccess,
    },
    BatchOk {
        request_id: Uuid,
        command: CommandKind,
        operations: Vec<BlockOperation>,
    },
    BlockCreated {
        id: Uuid,
        #[serde(rename = "type")]
        block_type: Uuid,
        author: Uuid,
        snapshot: Vec<u8>,
        snapshot_seq: u64,
        parent: BlockParent,
        properties: BTreeMap<Uuid, Vec<u8>>,
        access: BlockAccess,
    },
    Error {
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<Uuid>,
        #[serde(skip_serializing_if = "Option::is_none")]
        command: Option<CommandKind>,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<Uuid>,
        code: ErrorCode,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        expected_seq: Option<u64>,
    },
    BlockUpdated {
        id: Uuid,
        properties: BTreeMap<Uuid, Vec<u8>>,
        operation: OperationRecord,
    },
    BatchUpdated {
        operations: Vec<BlockOperation>,
    },
    /// A presence value changed for a block being watched. `client_id`
    /// identifies which watcher it belongs to, never the recipient's own
    /// connection: the server never echoes a client's presence back to
    /// itself. `data` is `None` when the value was cleared, either
    /// explicitly or because that client stopped watching the block.
    Presence {
        id: Uuid,
        client_id: ClientId,
        presence_id: Uuid,
        data: Option<Vec<u8>>,
    },
    BlockPropertyUpdated {
        id: Uuid,
        key: Uuid,
        value: Vec<u8>,
    },
    References {
        request_id: Uuid,
        list: BlockReferenceList,
        blocks: Vec<BlockReference>,
    },
    ReferencesUpdated {
        list: BlockReferenceList,
        blocks: Vec<BlockReference>,
    },
    BlockAccessList {
        request_id: Uuid,
        command: CommandKind,
        id: Uuid,
        entries: Vec<BlockAccessEntry>,
    },
}

impl ServerMessage {
    pub fn id(&self) -> Option<Uuid> {
        match self {
            Self::Ok { id, .. }
            | Self::ReadBlock { id, .. }
            | Self::BlockCreated { id, .. }
            | Self::BlockUpdated { id, .. }
            | Self::BlockPropertyUpdated { id, .. }
            | Self::BlockAccessList { id, .. }
            | Self::Presence { id, .. } => Some(*id),
            Self::BatchOk { .. }
            | Self::BatchUpdated { .. }
            | Self::References { .. }
            | Self::ReferencesUpdated { .. } => None,
            Self::Error { id, .. } => *id,
        }
    }
}
