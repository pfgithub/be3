use block::{Block, NoHistory};
use game_api::GameAction;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

                                                                           
                                                                           
                                                                   
                                                                         
   
                                                                         
                                                                         
                                                                            
                                                           
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct DeterministicGame {
    game: String,
    display_name: String,
    actions: Vec<GameAction>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum DeterministicGameOperation {
    Append { action: Vec<u8> },
}

impl DeterministicGame {
    pub fn new(game: String, display_name: String) -> Self {
        Self {
            game,
            display_name,
            actions: Vec::new(),
        }
    }

    pub fn game(&self) -> &str {
        &self.game
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
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

    fn implicit_name(&self) -> Option<String> {
        if self.display_name.is_empty() {
            return None;
        }
        Some(self.display_name.clone())
    }
}

#[cfg(test)]
mod tests;
