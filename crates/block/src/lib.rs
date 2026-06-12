use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

pub trait Block: Clone + Serialize + DeserializeOwned + Send + Sync + 'static {
    type Operation: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;

    const TYPE_ID: Uuid;
    const CRDT: bool = false;

    fn apply_operation(block: &mut Self, operation: &Self::Operation);

    fn transform_operation(_local: &mut Self::Operation, _remote: &Self::Operation) {}
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct OperationRecord {
    pub seq: u64,
    pub operation_id: Uuid,
    pub operation: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BlockOperation {
    pub id: Uuid,
    pub operation: OperationRecord,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BlockUpdate {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    pub operation_id: Uuid,
    pub operation: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    CreateBlock,
    UpdateBlock,
    UpdateBatch,
    ReadBlock,
    UnwatchBlock,
    PostPresence,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BlockAlreadyExists,
    BlockNotFound,
    ConflictingOperationId,
    InvalidMessage,
    InvalidSeq,
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
        watch: bool,
    },
    UpdateBlock {
        request_id: Uuid,
        id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
        operation_id: Uuid,
        operation: Vec<u8>,
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
}

impl ClientMessage {
    pub fn request_id(&self) -> Uuid {
        match self {
            Self::CreateBlock { request_id, .. }
            | Self::UpdateBlock { request_id, .. }
            | Self::UpdateBatch { request_id, .. }
            | Self::ReadBlock { request_id, .. }
            | Self::UnwatchBlock { request_id, .. }
            | Self::PostPresence { request_id, .. } => *request_id,
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
        snapshot: Vec<u8>,
        snapshot_seq: u64,
        operations: Vec<OperationRecord>,
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
        operation: OperationRecord,
    },
    BatchUpdated {
        operations: Vec<BlockOperation>,
    },
    Presence {
        id: Uuid,
        data: Vec<u8>,
    },
}

impl ServerMessage {
    pub fn id(&self) -> Option<Uuid> {
        match self {
            Self::Ok { id, .. }
            | Self::ReadBlock { id, .. }
            | Self::BlockUpdated { id, .. }
            | Self::Presence { id, .. } => Some(*id),
            Self::BatchOk { .. } | Self::BatchUpdated { .. } => None,
            Self::Error { id, .. } => *id,
        }
    }
}
