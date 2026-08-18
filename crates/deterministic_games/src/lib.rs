use std::cell::Cell;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod crazy_8s;
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
/// which player is looking, what they should see. The log is the only state:
/// implementations recompute everything from it on every call, typically by
/// driving a `GameHelper` (see the `tic_tac_toe` module for an example).
pub trait Game {
    fn show(&self, actions: &[GameAction], player: Uuid) -> GameScreen;
}

/// A concrete action a game offers to a player. `label` renders the button
/// text for one specific value, so it can depend on that value's own fields
/// (e.g. which cell a move targets) rather than being fixed per action type.
pub trait ActionLabel {
    fn label(&self) -> String;
}

/// Drives a game written as a single straight-line function over its action
/// log. The function calls `action` wherever it is waiting on a player: each
/// call replays forward through the log, accepting the next entry that both
/// deserializes as `T` and appears in the acting player's own legal options,
/// and silently skipping anything else (out-of-turn actions, stale actions
/// from an earlier phase, tampered payloads). Once the log runs out, it
/// returns the screen for the viewing player instead of a value, which the
/// game function propagates out with `?` - so the whole game is just its own
/// control flow re-run from the top on every call to `Game::show`.
pub struct GameHelper<'a> {
    actions: &'a [GameAction],
    cursor: Cell<usize>,
    player: Uuid,
}

impl<'a> GameHelper<'a> {
    pub fn new(actions: &'a [GameAction], player: Uuid) -> Self {
        Self {
            actions,
            cursor: Cell::new(0),
            player,
        }
    }

    pub fn action<T>(
        &self,
        describe: impl Fn(Uuid) -> String,
        options: impl Fn(Uuid) -> Vec<T>,
    ) -> Result<T, GameScreen>
    where
        T: ActionLabel + Serialize + DeserializeOwned + PartialEq,
    {
        while let Some(entry) = self.actions.get(self.cursor.get()) {
            self.cursor.set(self.cursor.get() + 1);
            if let Ok(decoded) = bincode::deserialize::<T>(&entry.action) {
                if options(entry.actor)
                    .into_iter()
                    .any(|option| option == decoded)
                {
                    return Ok(decoded);
                }
            }
        }
        let actions = options(self.player)
            .into_iter()
            .map(|option| GameActionOption {
                label: option.label(),
                effect: bincode::serialize(&option).expect("action encoding is infallible"),
            })
            .collect();
        Err(GameScreen {
            description: describe(self.player),
            actions,
        })
    }
}
