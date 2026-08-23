use std::collections::HashSet;

use block::{Block, BlockHistory, HistoryDirection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block_ref::BlockRef;

                                                 
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "slot", rename_all = "snake_case")]
pub enum HotbarSlot {
                                                                              
    Builtin { tool: String },
                                                                
    Locked { name: String },
    Folder {
        name: String,
        slots: Vec<HotbarSlot>,
    },
                                                  
    Component { name: String, compiled: BlockRef },
}

impl HotbarSlot {
    fn collect_components(&self, into: &mut Vec<Uuid>) {
        match self {
            Self::Component { compiled, .. } => into.extend(compiled.as_direct()),
            Self::Folder { slots, .. } => {
                for slot in slots {
                    slot.collect_components(into);
                }
            }
            Self::Builtin { .. } | Self::Locked { .. } => {}
        }
    }

    fn collect_component_refs(&self, into: &mut Vec<BlockRef>) {
        match self {
            Self::Component { compiled, .. } => into.push(*compiled),
            Self::Folder { slots, .. } => {
                for slot in slots {
                    slot.collect_component_refs(into);
                }
            }
            Self::Builtin { .. } | Self::Locked { .. } => {}
        }
    }
}

                                                                                
                                                                          
                                                                         
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct Hotbar {
    slots: Vec<HotbarSlot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum HotbarOperation {
                                                                         
                                                                     
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

                                                              
    pub fn components(&self) -> Vec<Uuid> {
        let mut components = Vec::new();
        for slot in &self.slots {
            slot.collect_components(&mut components);
        }
        let mut seen = HashSet::new();
        components.retain(|compiled| seen.insert(*compiled));
        components
    }

    pub fn component_refs(&self) -> Vec<BlockRef> {
        let mut refs = Vec::new();
        for slot in &self.slots {
            slot.collect_component_refs(&mut refs);
        }
        let mut seen = HashSet::new();
        refs.retain(|compiled| seen.insert(*compiled));
        refs
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
