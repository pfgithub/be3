use block::{Block, NoHistory};
use game_api::GameAction;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct DeterministicGame {
    module: Uuid,
    actions: Vec<GameAction>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum DeterministicGameOperation {
    Append { action: Vec<u8> },
}

impl DeterministicGame {
    pub fn new(module: Uuid) -> Self {
        Self {
            module,
            actions: Vec::new(),
        }
    }

    pub fn module(&self) -> Uuid {
        self.module
    }

    pub fn actions(&self) -> &[GameAction] {
        &self.actions
    }
}

impl Block for DeterministicGame {
    type Operation = DeterministicGameOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6465_742d_6761_6d65_2d62_6c6f_636b_0001);

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        Self::apply_authored_operation(block, operation, Uuid::nil());
    }

    fn apply_authored_operation(block: &mut Self, operation: &Self::Operation, author: Uuid) {
        match operation {
            DeterministicGameOperation::Append { action } => block.actions.push(GameAction {
                actor: author,
                action: action.clone(),
            }),
        }
    }

    fn references(&self) -> Vec<Uuid> {
        vec![self.module]
    }
}

#[cfg(test)]
mod tests;
