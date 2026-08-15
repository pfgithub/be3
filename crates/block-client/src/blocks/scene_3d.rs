use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Scene3D;

impl Scene3D {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum Scene3DOperation {}

impl Block for Scene3D {
    type Operation = Scene3DOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x3364_2d73_6365_6e65_2d62_6c6f_636b_3031);

    fn apply_operation(_block: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

#[cfg(test)]
mod tests;
