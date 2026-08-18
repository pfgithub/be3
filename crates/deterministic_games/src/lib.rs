use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod tic_tac_toe;

/// One entry in a deterministic game's append-only action log. `actor` is
/// always the operation's server-verified author, never something a client
/// can claim for itself through `action`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GameAction {
    pub actor: Uuid,
    pub action: Vec<u8>,
}

/// What a game currently looks like to one viewer: a description of the
/// state, and the actions they may take from here (empty if it is not their
/// turn, or the game has ended).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameScreen {
    pub description: String,
    pub actions: Vec<GameActionOption>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameActionOption {
    pub label: String,
    pub effect: Vec<u8>,
}

/// A deterministic game's entire logic: given the full action log so far and
/// which player is looking, what they should see. The log is the only state
/// - implementations recompute everything from it on every call.
pub trait Game {
    fn show(&self, actions: &[GameAction], player: Uuid) -> GameScreen;
}
