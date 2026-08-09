use std::collections::HashSet;

use block::{Block, BlockHistory, HistoryDirection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One entry in the logic editor's tool palette.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "slot", rename_all = "snake_case")]
pub enum HotbarSlot {
    /// A tool the editor provides itself, named by the editor's own tool key.
    Builtin { tool: String },
    /// A placeholder for a tool that has not been unlocked yet.
    Locked { name: String },
    Folder {
        name: String,
        slots: Vec<HotbarSlot>,
    },
    /// A compiled logic block pinned for placing.
    Component { name: String, compiled: Uuid },
}

impl HotbarSlot {
    fn collect_components(&self, into: &mut Vec<Uuid>) {
        match self {
            Self::Component { compiled, .. } => into.push(*compiled),
            Self::Folder { slots, .. } => {
                for slot in slots {
                    slot.collect_components(into);
                }
            }
            Self::Builtin { .. } | Self::Locked { .. } => {}
        }
    }
}

/// The tool palette shared by the grids of one game. It is registered under the
/// workspace's root `Settings` block rather than living inside a grid, so
/// pinning a component once offers it in every circuit built afterwards.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct Hotbar {
    slots: Vec<HotbarSlot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum HotbarOperation {
    /// Replaces the whole tree. Slots are dragged between folders as one
    /// gesture, so there is no smaller unit of change worth sending.
    SetSlots { slots: Vec<HotbarSlot> },
}

pub struct HotbarHistory;

pub struct HotbarHistoryAction {
    before: Vec<HotbarSlot>,
    after: Vec<HotbarSlot>,
}

impl Hotbar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_slots(slots: Vec<HotbarSlot>) -> Self {
        Self { slots }
    }

    pub fn slots(&self) -> &[HotbarSlot] {
        &self.slots
    }

    /// The compiled logic blocks pinned anywhere in the tree.
    pub fn components(&self) -> Vec<Uuid> {
        let mut components = Vec::new();
        for slot in &self.slots {
            slot.collect_components(&mut components);
        }
        let mut seen = HashSet::new();
        components.retain(|compiled| seen.insert(*compiled));
        components
    }
}

impl Block for Hotbar {
    type Operation = HotbarOperation;
    type History = HotbarHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6c6f_6769_632d_686f_7462_6172_0101_0101);

    fn apply_operation(hotbar: &mut Self, operation: &Self::Operation) {
        match operation {
            HotbarOperation::SetSlots { slots } => hotbar.slots.clone_from(slots),
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.components()
    }
}

impl BlockHistory<Hotbar> for HotbarHistory {
    type Action = HotbarHistoryAction;
    type Snapshot = Hotbar;

    fn snapshot(block: &Hotbar) -> Self::Snapshot {
        block.clone()
    }

    fn action(
        before: Hotbar,
        after: &Hotbar,
        _operations: &[HotbarOperation],
    ) -> Option<Self::Action> {
        (before.slots != after.slots).then(|| HotbarHistoryAction {
            before: before.slots,
            after: after.slots.clone(),
        })
    }

    fn action_bytes(action: &Self::Action) -> usize {
        (action.before.len() + action.after.len()) * 128
    }

    fn operations(
        _current: &Hotbar,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) -> Vec<HotbarOperation> {
        vec![HotbarOperation::SetSlots {
            slots: if direction == HistoryDirection::Redo {
                action.after.clone()
            } else {
                action.before.clone()
            },
        }]
    }
}

#[cfg(test)]
mod tests;
