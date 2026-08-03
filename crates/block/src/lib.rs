use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Account {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRole {
    Administrator,
    Editor,
    Viewer,
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
    InvalidEmail,
    InvalidMessage,
    InvalidName,
    InvitationAlreadyExists,
    InvitationNotFound,
    PermissionDenied,
    StorageError,
    UnsupportedRole,
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
    },
    Login {
        request_id: Uuid,
        email: String,
    },
    ListWorkspaces {
        request_id: Uuid,
        account_id: Uuid,
    },
    CreateWorkspace {
        request_id: Uuid,
        account_id: Uuid,
        name: String,
    },
    ListInvitations {
        request_id: Uuid,
        account_id: Uuid,
    },
    Invite {
        request_id: Uuid,
        account_id: Uuid,
        workspace_id: Uuid,
        email: String,
        role: WorkspaceRole,
    },
    RespondInvitation {
        request_id: Uuid,
        account_id: Uuid,
        invitation_id: Uuid,
        accept: bool,
    },
}

impl ManagementClientMessage {
    pub fn request_id(&self) -> Uuid {
        match self {
            Self::Register { request_id, .. }
            | Self::Login { request_id, .. }
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

    fn implicit_name(&self) -> String;

    fn transform_operation(_local: &mut Self::Operation, _remote: &Self::Operation) {}

    fn references(&self) -> Vec<Uuid> {
        Vec::new()
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
    pub name: String,
    pub operation: OperationRecord,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BlockUpdate {
    pub id: Uuid,
    pub implicit_name: String,
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
    pub name: String,
    pub parent: BlockParent,
    pub references: usize,
}

pub const MAX_NAME_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    CreateBlock,
    UpdateBlock,
    UpdateBatch,
    ReadBlock,
    UnwatchBlock,
    PostPresence,
    SetBlockParent,
    SetBlockName,
    ListReferences,
    UnwatchReferences,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BlockAlreadyExists,
    BlockNotFound,
    ConflictingOperationId,
    InvalidMessage,
    InvalidSeq,
    ParentCycle,
    ParentMissingReference,
    ReferencedBlockNotFound,
    StorageError,
    UnsupportedMessage,
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
        implicit_name: String,
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
        implicit_name: String,
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
    PostPresence {
        request_id: Uuid,
        id: Uuid,
        data: Vec<u8>,
    },
    SetBlockParent {
        request_id: Uuid,
        id: Uuid,
        parent: BlockParent,
    },
    SetBlockName {
        request_id: Uuid,
        id: Uuid,
        name: String,
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
}

impl ClientMessage {
    pub fn request_id(&self) -> Uuid {
        match self {
            Self::CreateBlock { request_id, .. }
            | Self::UpdateBlock { request_id, .. }
            | Self::UpdateBatch { request_id, .. }
            | Self::ReadBlock { request_id, .. }
            | Self::UnwatchBlock { request_id, .. }
            | Self::PostPresence { request_id, .. }
            | Self::SetBlockParent { request_id, .. }
            | Self::SetBlockName { request_id, .. }
            | Self::ListReferences { request_id, .. }
            | Self::UnwatchReferences { request_id, .. } => *request_id,
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
        name: String,
    },
    BatchOk {
        request_id: Uuid,
        command: CommandKind,
        operations: Vec<BlockOperation>,
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
        name: String,
        operation: OperationRecord,
    },
    BatchUpdated {
        operations: Vec<BlockOperation>,
    },
    Presence {
        id: Uuid,
        data: Vec<u8>,
    },
    BlockNameUpdated {
        id: Uuid,
        name: String,
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
}

impl ServerMessage {
    pub fn id(&self) -> Option<Uuid> {
        match self {
            Self::Ok { id, .. }
            | Self::ReadBlock { id, .. }
            | Self::BlockUpdated { id, .. }
            | Self::BlockNameUpdated { id, .. }
            | Self::Presence { id, .. } => Some(*id),
            Self::BatchOk { .. }
            | Self::BatchUpdated { .. }
            | Self::References { .. }
            | Self::ReferencesUpdated { .. } => None,
            Self::Error { id, .. } => *id,
        }
    }
}
