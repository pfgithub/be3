use std::cell::Cell;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod build;
pub mod guest;

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
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GameScreen {
    pub description: String,
    pub actions: Vec<GameActionOption>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GameActionOption {
    pub label: String,
    pub effect: Vec<u8>,
}

/// Everything a game module is given to answer one `show` call. The host
/// writes this into the module's memory and the module decodes it there, so
/// it is the only shape both sides have to agree on beyond the exported
/// functions themselves.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GameRequest {
    pub actions: Vec<GameAction>,
    pub player: Uuid,
}

/// Drives a game written as a single straight-line function over its action
/// log. The function calls `action` wherever it is waiting on a player,
/// passing a closure that offers each of that player's legal moves in turn
/// by calling `choose(label)` for it: `choose` returns `true` and the caller
/// mutates its own local game state right there, inline, if that move is the
/// one the log records next for that actor; otherwise it returns `false` and
/// nothing happens. A move is identified purely by its position among the
/// `choose` calls reached for its actor - never by its label or any encoded
/// value - so the log entry that records it, and the `effect` bytes handed
/// back to a client offered it, are just that position as a single number.
/// Once the log runs out, `action` returns the screen for the viewing player
/// instead, which the game function propagates out with `?` - so the whole
/// game is just its own control flow re-run from the top on every call to
/// the module's `show` export.
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

    pub fn action(
        &self,
        describe: impl Fn(Uuid) -> String,
        mut body: impl FnMut(Uuid, &mut dyn FnMut(&str) -> bool),
    ) -> Result<(), GameScreen> {
        while let Some(entry) = self.actions.get(self.cursor.get()) {
            self.cursor.set(self.cursor.get() + 1);
            let Ok(target) = bincode::deserialize::<u32>(&entry.action) else {
                continue;
            };
            let mut seen = 0u32;
            let mut matched = false;
            body(entry.actor, &mut |_label: &str| {
                let is_target = !matched && seen == target;
                seen += 1;
                matched |= is_target;
                is_target
            });
            if matched {
                return Ok(());
            }
        }
        let mut index = 0u32;
        let mut actions = Vec::new();
        body(self.player, &mut |label: &str| {
            actions.push(GameActionOption {
                label: label.to_owned(),
                effect: bincode::serialize(&index).expect("index encoding is infallible"),
            });
            index += 1;
            false
        });
        Err(GameScreen {
            description: describe(self.player),
            actions,
        })
    }
}
