use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

pub trait Block: Clone + Serialize + DeserializeOwned + Send + Sync + 'static {
    type Operation: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;

    const TYPE_ID: Uuid;
    const CRDT: bool = false;

    fn apply_operation(block: &mut Self, operation: &Self::Operation);

    fn implicit_name(&self) -> String;

    fn transform_operation(_local: &mut Self::Operation, _remote: &Self::Operation) {}

    fn references(&self) -> Vec<Uuid> {
        Vec::new()
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
