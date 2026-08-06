use std::collections::{BTreeMap, HashSet};

use block::{Block, BlockHistory, HistoryDirection};
use logicgame::challenges::{ChallengeId, CHALLENGES};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One level: a built-in challenge, the grids that have been started for it,
/// and whether any of them has passed.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Level {
    pub challenge: ChallengeId,
    /// Logic grid blocks holding attempts at this level, in the order they
    /// were created.
    pub solutions: Vec<Uuid>,
    pub completed: bool,
}

impl Level {
    fn new(challenge: ChallengeId) -> Self {
        Self {
            challenge,
            solutions: Vec::new(),
            completed: false,
        }
    }
}

/// Which of a quiz problem's blanks have been filled in. A blank is `None`, and
/// the rows are as long as the player's editor made them; the level's own
/// problems decide what counts as correct.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct QuizProblem {
    pub carries: Vec<Option<bool>>,
    pub sums: Vec<Option<bool>>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuizRow {
    Carries,
    Sums,
}

/// The list of levels and the answers to the levels that are worked through on
/// paper rather than on a grid. The palette its circuits are built from is not
/// part of it: the editor finds the shared hotbar at the root of the block tree.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct LogicGame {
    levels: Vec<Level>,
    #[serde(default)]
    quiz: BTreeMap<usize, QuizProblem>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum LogicGameOperation {
    InsertSolution {
        challenge: ChallengeId,
        solution: Uuid,
        index: usize,
    },
    RemoveSolution {
        challenge: ChallengeId,
        solution: Uuid,
    },
    SetCompleted {
        challenge: ChallengeId,
        completed: bool,
    },
    SetQuizRow {
        problem: usize,
        row: QuizRow,
        values: Vec<Option<bool>>,
    },
}

pub struct LogicGameHistory;

pub struct LogicGameHistoryAction {
    changes: Vec<LogicGameHistoryChange>,
}

enum LogicGameHistoryChange {
    Solution {
        challenge: ChallengeId,
        solution: Uuid,
        index: usize,
        added: bool,
    },
    Completed {
        challenge: ChallengeId,
        before: bool,
        after: bool,
    },
    QuizRow {
        problem: usize,
        row: QuizRow,
        before: Vec<Option<bool>>,
        after: Vec<Option<bool>>,
    },
}

impl Default for LogicGame {
    fn default() -> Self {
        Self::new()
    }
}

impl LogicGame {
    /// A new game with every built-in challenge listed and nothing solved.
    pub fn new() -> Self {
        Self {
            levels: CHALLENGES.into_iter().map(Level::new).collect(),
            quiz: BTreeMap::new(),
        }
    }

    pub fn levels(&self) -> &[Level] {
        &self.levels
    }

    pub fn level(&self, challenge: ChallengeId) -> Option<&Level> {
        self.levels
            .iter()
            .find(|level| level.challenge == challenge)
    }

    pub fn quiz(&self, problem: usize) -> Option<&QuizProblem> {
        self.quiz.get(&problem)
    }

    fn level_mut(&mut self, challenge: ChallengeId) -> Option<&mut Level> {
        self.levels
            .iter_mut()
            .find(|level| level.challenge == challenge)
    }
}

impl Block for LogicGame {
    type Operation = LogicGameOperation;
    type History = LogicGameHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6c6f_6769_632d_6761_6d65_2d62_6c6b_0101);

    fn apply_operation(game: &mut Self, operation: &Self::Operation) {
        match operation {
            LogicGameOperation::InsertSolution {
                challenge,
                solution,
                index,
            } => {
                if let Some(level) = game.level_mut(*challenge) {
                    if !level.solutions.contains(solution) {
                        let index = (*index).min(level.solutions.len());
                        level.solutions.insert(index, *solution);
                    }
                }
            }
            LogicGameOperation::RemoveSolution {
                challenge,
                solution,
            } => {
                if let Some(level) = game.level_mut(*challenge) {
                    level.solutions.retain(|existing| existing != solution);
                }
            }
            LogicGameOperation::SetCompleted {
                challenge,
                completed,
            } => {
                if let Some(level) = game.level_mut(*challenge) {
                    level.completed = *completed;
                }
            }
            LogicGameOperation::SetQuizRow {
                problem,
                row,
                values,
            } => {
                let answers = game.quiz.entry(*problem).or_default();
                match row {
                    QuizRow::Carries => answers.carries.clone_from(values),
                    QuizRow::Sums => answers.sums.clone_from(values),
                }
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        self.levels
            .iter()
            .flat_map(|level| level.solutions.iter().copied())
            .filter(|reference| seen.insert(*reference))
            .collect()
    }
}

impl BlockHistory<LogicGame> for LogicGameHistory {
    type Action = LogicGameHistoryAction;
    type Snapshot = LogicGame;

    fn snapshot(block: &LogicGame) -> Self::Snapshot {
        block.clone()
    }

    fn action(
        before: LogicGame,
        _after: &LogicGame,
        operations: &[LogicGameOperation],
    ) -> Option<Self::Action> {
        let mut current = before;
        let mut changes = Vec::new();
        for operation in operations {
            let mut next = current.clone();
            LogicGame::apply_operation(&mut next, operation);
            if next != current {
                changes.extend(change_between(&current, &next, operation));
            }
            current = next;
        }
        (!changes.is_empty()).then_some(LogicGameHistoryAction { changes })
    }

    fn action_bytes(action: &Self::Action) -> usize {
        action.changes.len() * 128
    }

    fn operations(
        _current: &LogicGame,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) -> Vec<LogicGameOperation> {
        let to_after = direction == HistoryDirection::Redo;
        let changes: Box<dyn Iterator<Item = &LogicGameHistoryChange> + '_> = if to_after {
            Box::new(action.changes.iter())
        } else {
            Box::new(action.changes.iter().rev())
        };
        changes
            .map(|change| match change {
                LogicGameHistoryChange::Solution {
                    challenge,
                    solution,
                    index,
                    added,
                } => {
                    if *added == to_after {
                        LogicGameOperation::InsertSolution {
                            challenge: *challenge,
                            solution: *solution,
                            index: *index,
                        }
                    } else {
                        LogicGameOperation::RemoveSolution {
                            challenge: *challenge,
                            solution: *solution,
                        }
                    }
                }
                LogicGameHistoryChange::Completed {
                    challenge,
                    before,
                    after,
                } => LogicGameOperation::SetCompleted {
                    challenge: *challenge,
                    completed: if to_after { *after } else { *before },
                },
                LogicGameHistoryChange::QuizRow {
                    problem,
                    row,
                    before,
                    after,
                } => LogicGameOperation::SetQuizRow {
                    problem: *problem,
                    row: *row,
                    values: if to_after {
                        after.clone()
                    } else {
                        before.clone()
                    },
                },
            })
            .collect()
    }
}

fn change_between(
    before: &LogicGame,
    after: &LogicGame,
    operation: &LogicGameOperation,
) -> Option<LogicGameHistoryChange> {
    match operation {
        LogicGameOperation::InsertSolution {
            challenge,
            solution,
            ..
        } => {
            let index = after
                .level(*challenge)?
                .solutions
                .iter()
                .position(|existing| existing == solution)?;
            Some(LogicGameHistoryChange::Solution {
                challenge: *challenge,
                solution: *solution,
                index,
                added: true,
            })
        }
        LogicGameOperation::RemoveSolution {
            challenge,
            solution,
        } => {
            let index = before
                .level(*challenge)?
                .solutions
                .iter()
                .position(|existing| existing == solution)?;
            Some(LogicGameHistoryChange::Solution {
                challenge: *challenge,
                solution: *solution,
                index,
                added: false,
            })
        }
        LogicGameOperation::SetCompleted { challenge, .. } => {
            Some(LogicGameHistoryChange::Completed {
                challenge: *challenge,
                before: before.level(*challenge)?.completed,
                after: after.level(*challenge)?.completed,
            })
        }
        LogicGameOperation::SetQuizRow { problem, row, .. } => {
            let read = |game: &LogicGame| {
                game.quiz(*problem).map_or_else(Vec::new, |answers| {
                    match row {
                        QuizRow::Carries => &answers.carries,
                        QuizRow::Sums => &answers.sums,
                    }
                    .clone()
                })
            };
            Some(LogicGameHistoryChange::QuizRow {
                problem: *problem,
                row: *row,
                before: read(before),
                after: read(after),
            })
        }
    }
}

#[cfg(test)]
mod tests;
