use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileTree {}

impl FileTree {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum FileTreeOperation {}

impl Block for FileTree {
    type Operation = FileTreeOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6669_6c65_2d74_7265_652d_626c_6f63_6b01);

    fn apply_operation(_tree: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

#[cfg(test)]
mod tests;
