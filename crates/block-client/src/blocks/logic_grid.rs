use std::collections::HashSet;

use block::{Block, BlockHistory, HistoryDirection};
use logicgame::challenges::ChallengeId;
use logicgame::grid::{
    Component, ComponentId, ComponentKind, ComponentOrientation, LogicGrid as Grid, Point, Wire,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One circuit: the components placed on the grid and the wires between them.
/// The palette it is edited with is not part of it: the editor finds the shared
/// hotbar at the root of the block tree instead.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct LogicGrid {
    grid: Grid,
    /// The level this grid is an attempt at. A grid built outside a game has
    /// none, and is edited without a goal or a test to run against.
    #[serde(default)]
    challenge: Option<ChallengeId>,
    /// Whether this grid has passed its level's test. The game reads it back
    /// off its solutions rather than being told by the editor that ran them.
    #[serde(default)]
    completed: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum LogicGridOperation {
    /// Adds a component under an ID the sender allocated with
    /// [`LogicGrid::next_component_id`]. Repeats are ignored, so replaying the
    /// operation - from history or from a peer - never duplicates it.
    AddComponent {
        component: Component,
    },
    RemoveComponent {
        id: ComponentId,
    },
    MoveComponent {
        id: ComponentId,
        position: Point,
    },
    OrientComponent {
        id: ComponentId,
        orientation: ComponentOrientation,
    },
    SetComponentKind {
        id: ComponentId,
        kind: ComponentKind,
    },
    SetStorageValue {
        id: ComponentId,
        value: u64,
    },
    AddWire {
        wire: Wire,
    },
    /// Clears every wire covering `wire`'s span, splitting the segments that
    /// only partly overlap it.
    RemoveWire {
        wire: Wire,
    },
    /// Removes one whole segment, exactly as it is stored.
    RemoveWireSegment {
        wire: Wire,
    },
    SetCompleted {
        completed: bool,
    },
}

pub struct LogicGridHistory;

pub struct LogicGridHistoryAction {
    changes: Vec<LogicGridHistoryChange>,
}

enum LogicGridHistoryChange {
    AddComponent(Component),
    RemoveComponent(Component),
    UpdateComponent {
        before: Component,
        after: Component,
    },
    /// Wire edits merge and split neighbouring segments, so what a single
    /// operation changed is recorded as the segments that left and joined the
    /// grid rather than as an inverse operation.
    Wires {
        removed: Vec<Wire>,
        added: Vec<Wire>,
    },
    Completed {
        before: bool,
        after: bool,
    },
}

impl LogicGrid {
    pub fn new() -> Self {
        Self::default()
    }

    /// Wraps a circuit that was built elsewhere, such as one being imported or
    /// one a test set up directly.
    pub fn from_grid(grid: Grid) -> Self {
        Self {
            grid,
            challenge: None,
            completed: false,
        }
    }

    /// Marks the grid as an attempt at `challenge`.
    #[must_use]
    pub fn with_challenge(mut self, challenge: ChallengeId) -> Self {
        self.challenge = Some(challenge);
        self
    }

    /// An empty grid started for `challenge`.
    pub fn for_challenge(challenge: ChallengeId) -> Self {
        Self {
            grid: Grid::new(),
            challenge: Some(challenge),
            completed: false,
        }
    }

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    pub fn challenge(&self) -> Option<ChallengeId> {
        self.challenge
    }

    pub fn completed(&self) -> bool {
        self.completed
    }

    /// The ID the next [`LogicGridOperation::AddComponent`] should carry.
    pub fn next_component_id(&self) -> ComponentId {
        self.grid.next_component_id()
    }

    /// The compiled logic blocks the placed subcomponents call.
    pub fn called_blocks(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        self.grid
            .components()
            .filter_map(|component| match component.kind {
                ComponentKind::Subcomponent { compiled, .. } => Some(compiled),
                _ => None,
            })
            .filter(|compiled| seen.insert(*compiled))
            .collect()
    }
}

impl Block for LogicGrid {
    type Operation = LogicGridOperation;
    type History = LogicGridHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6c6f_6769_632d_6772_6964_2d62_6c6b_0101);

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        let grid = &mut block.grid;
        match operation {
            LogicGridOperation::AddComponent { component } => {
                grid.insert_component(component.clone());
            }
            LogicGridOperation::RemoveComponent { id } => {
                grid.remove_component(*id);
            }
            LogicGridOperation::MoveComponent { id, position } => {
                grid.set_component_position(*id, *position);
            }
            LogicGridOperation::OrientComponent { id, orientation } => {
                grid.set_component_orientation(*id, *orientation);
            }
            LogicGridOperation::SetComponentKind { id, kind } => {
                grid.set_component_kind(*id, kind.clone());
            }
            LogicGridOperation::SetStorageValue { id, value } => {
                grid.set_storage_value(*id, *value);
            }
            LogicGridOperation::AddWire { wire } => {
                grid.add_wire(*wire);
            }
            LogicGridOperation::RemoveWire { wire } => {
                grid.remove_wire(*wire);
            }
            LogicGridOperation::RemoveWireSegment { wire } => {
                grid.remove_wire_segment(*wire);
            }
            LogicGridOperation::SetCompleted { completed } => block.completed = *completed,
        }
    }

    fn implicit_name(&self) -> String {
        "Logic Grid".into()
    }

    fn references(&self) -> Vec<Uuid> {
        self.called_blocks()
    }
}

impl BlockHistory<LogicGrid> for LogicGridHistory {
    type Action = LogicGridHistoryAction;
    type Snapshot = LogicGrid;

    fn snapshot(block: &LogicGrid) -> Self::Snapshot {
        block.clone()
    }

    fn action(
        before: LogicGrid,
        after: &LogicGrid,
        operations: &[LogicGridOperation],
    ) -> Option<Self::Action> {
        let mut current = before;
        let mut changes = Vec::new();
        for operation in operations {
            let mut next = current.clone();
            LogicGrid::apply_operation(&mut next, operation);
            changes.extend(change_between(&current, &next, operation));
            current = next;
        }
        debug_assert_eq!(&current, after);
        (!changes.is_empty()).then_some(LogicGridHistoryAction { changes })
    }

    fn action_bytes(action: &Self::Action) -> usize {
        action
            .changes
            .iter()
            .map(|change| match change {
                LogicGridHistoryChange::AddComponent(_)
                | LogicGridHistoryChange::RemoveComponent(_) => 256,
                LogicGridHistoryChange::UpdateComponent { .. } => 512,
                LogicGridHistoryChange::Wires { removed, added } => {
                    (removed.len() + added.len()) * size_of::<Wire>()
                }
                LogicGridHistoryChange::Completed { .. } => 2,
            })
            .sum()
    }

    fn operations(
        _current: &LogicGrid,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) -> Vec<LogicGridOperation> {
        let to_after = direction == HistoryDirection::Redo;
        let changes: Box<dyn Iterator<Item = &LogicGridHistoryChange> + '_> = if to_after {
            Box::new(action.changes.iter())
        } else {
            Box::new(action.changes.iter().rev())
        };
        changes
            .flat_map(|change| match change {
                LogicGridHistoryChange::AddComponent(component) => {
                    component_operations(component, to_after)
                }
                LogicGridHistoryChange::RemoveComponent(component) => {
                    component_operations(component, !to_after)
                }
                LogicGridHistoryChange::UpdateComponent { before, after } => {
                    let desired = if to_after { after } else { before };
                    vec![
                        LogicGridOperation::MoveComponent {
                            id: desired.id,
                            position: desired.position,
                        },
                        LogicGridOperation::OrientComponent {
                            id: desired.id,
                            orientation: desired.orientation,
                        },
                        LogicGridOperation::SetComponentKind {
                            id: desired.id,
                            kind: desired.kind.clone(),
                        },
                    ]
                }
                LogicGridHistoryChange::Wires { removed, added } => {
                    // The segments to take out are removed before the ones to
                    // put back are added, so every removal still matches a
                    // segment exactly as the grid stores it.
                    let (take_out, put_back) = if to_after {
                        (removed, added)
                    } else {
                        (added, removed)
                    };
                    take_out
                        .iter()
                        .map(|wire| LogicGridOperation::RemoveWireSegment { wire: *wire })
                        .chain(
                            put_back
                                .iter()
                                .map(|wire| LogicGridOperation::AddWire { wire: *wire }),
                        )
                        .collect()
                }
                LogicGridHistoryChange::Completed { before, after } => {
                    vec![LogicGridOperation::SetCompleted {
                        completed: if to_after { *after } else { *before },
                    }]
                }
            })
            .collect()
    }
}

/// Restoring a component takes its kind and orientation as well as its ID, so
/// adding it back rebuilds exactly what was removed.
fn component_operations(component: &Component, add: bool) -> Vec<LogicGridOperation> {
    if add {
        vec![LogicGridOperation::AddComponent {
            component: component.clone(),
        }]
    } else {
        vec![LogicGridOperation::RemoveComponent { id: component.id }]
    }
}

fn change_between(
    before: &LogicGrid,
    after: &LogicGrid,
    operation: &LogicGridOperation,
) -> Option<LogicGridHistoryChange> {
    match operation {
        LogicGridOperation::AddComponent { component } => before
            .grid
            .component(component.id)
            .is_none()
            .then(|| LogicGridHistoryChange::AddComponent(component.clone())),
        LogicGridOperation::RemoveComponent { id } => before
            .grid
            .component(*id)
            .cloned()
            .map(LogicGridHistoryChange::RemoveComponent),
        LogicGridOperation::MoveComponent { id, .. }
        | LogicGridOperation::OrientComponent { id, .. }
        | LogicGridOperation::SetComponentKind { id, .. }
        | LogicGridOperation::SetStorageValue { id, .. } => {
            let previous = before.grid.component(*id)?;
            let current = after.grid.component(*id)?;
            (previous != current).then(|| LogicGridHistoryChange::UpdateComponent {
                before: previous.clone(),
                after: current.clone(),
            })
        }
        LogicGridOperation::AddWire { .. }
        | LogicGridOperation::RemoveWire { .. }
        | LogicGridOperation::RemoveWireSegment { .. } => {
            let removed = difference(before.grid.wires(), after.grid.wires());
            let added = difference(after.grid.wires(), before.grid.wires());
            (!removed.is_empty() || !added.is_empty())
                .then_some(LogicGridHistoryChange::Wires { removed, added })
        }
        LogicGridOperation::SetCompleted { .. } => {
            (before.completed != after.completed).then_some(LogicGridHistoryChange::Completed {
                before: before.completed,
                after: after.completed,
            })
        }
    }
}

fn difference(wires: &[Wire], other: &[Wire]) -> Vec<Wire> {
    wires
        .iter()
        .filter(|wire| !other.contains(wire))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests;
