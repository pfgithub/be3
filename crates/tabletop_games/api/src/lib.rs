use std::cell::Cell;
use std::convert::Infallible;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod build;
pub mod cards;
pub mod guest;
pub mod table;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GameAction {
    pub actor: Uuid,
    pub action: Vec<u8>,
}

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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GameRequest {
    pub actions: Vec<GameAction>,
    pub player: Uuid,
}

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

    pub fn turn(
        &self,
        whose: Uuid,
        yours: &str,
        theirs: &str,
        mut choices: impl FnMut(&mut dyn FnMut(&str) -> bool),
    ) -> Result<(), GameScreen> {
        self.action(
            |player| {
                if player == whose {
                    yours.to_owned()
                } else {
                    theirs.to_owned()
                }
            },
            |player, choose| {
                if player == whose {
                    choices(choose);
                }
            },
        )
    }

    pub fn gather(&self, minimum: usize) -> Result<Vec<Uuid>, GameScreen> {
        let mut players: Vec<Uuid> = Vec::new();
        loop {
            let joined = players.clone();
            let mut started = false;
            self.action(
                move |player| {
                    if !joined.contains(&player) {
                        "Join the game".to_owned()
                    } else if joined.len() < minimum {
                        "Waiting for another player to join...".to_owned()
                    } else {
                        format!("{} players joined - start when ready", joined.len())
                    }
                },
                |player, choose| {
                    if !players.contains(&player) {
                        if choose("Join the game") {
                            players.push(player);
                        }
                    } else if players.len() >= minimum && choose("Start the game") {
                        started = true;
                    }
                },
            )?;
            if started {
                return Ok(players);
            }
        }
    }

    pub fn game_over(&self, describe: impl Fn(Uuid) -> String) -> Result<Infallible, GameScreen> {
        Err(GameScreen {
            description: describe(self.player),
            actions: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests;
