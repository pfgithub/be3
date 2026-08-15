use std::collections::HashSet;

use block::{Block, BlockHistory, HistoryDirection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block_ref::BlockRef;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct PresentationSlide {
    pub id: Uuid,
    pub block_id: BlockRef,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct Presentation {
    slides: Vec<PresentationSlide>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum PresentationOperation {
    Insert {
        slide: PresentationSlide,
        index: usize,
    },
    Remove {
        slide_id: Uuid,
    },
    Move {
        slide_id: Uuid,
        index: usize,
    },
    SetBlockId {
        slide_id: Uuid,
        block_id: BlockRef,
    },
}

pub struct PresentationHistory;

pub struct PresentationHistoryAction {
    changes: Vec<PresentationHistoryChange>,
}

enum PresentationHistoryChange {
    Insert {
        slide: PresentationSlide,
        index: usize,
    },
    Remove {
        slide: PresentationSlide,
        index: usize,
    },
    Move {
        slide_id: Uuid,
        before: usize,
        after: usize,
    },
    SetBlockId {
        slide_id: Uuid,
        before: BlockRef,
        after: BlockRef,
    },
}

impl Presentation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn slides(&self) -> &[PresentationSlide] {
        &self.slides
    }
}

impl Block for Presentation {
    type Operation = PresentationOperation;
    type History = PresentationHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7072_6573_656e_7461_7469_6f6e_0001);

    fn apply_operation(presentation: &mut Self, operation: &Self::Operation) {
        match operation {
            PresentationOperation::Insert { slide, index } => {
                if presentation
                    .slides
                    .iter()
                    .any(|existing| existing.id == slide.id)
                {
                    return;
                }
                presentation
                    .slides
                    .insert((*index).min(presentation.slides.len()), slide.clone());
            }
            PresentationOperation::Remove { slide_id } => {
                presentation.slides.retain(|slide| slide.id != *slide_id);
            }
            PresentationOperation::Move { slide_id, index } => {
                let Some(current) = presentation
                    .slides
                    .iter()
                    .position(|slide| slide.id == *slide_id)
                else {
                    return;
                };
                let slide = presentation.slides.remove(current);
                let index = (*index).min(presentation.slides.len());
                presentation.slides.insert(index, slide);
            }
            PresentationOperation::SetBlockId { slide_id, block_id } => {
                if let Some(slide) = presentation
                    .slides
                    .iter_mut()
                    .find(|slide| slide.id == *slide_id)
                {
                    slide.block_id = *block_id;
                }
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        self.slides
            .iter()
            .filter_map(|slide| slide.block_id.as_direct())
            .filter(|block_id| seen.insert(*block_id))
            .collect()
    }

    fn add_child(&self, block_id: Uuid) -> Option<Vec<Self::Operation>> {
        Some(vec![PresentationOperation::Insert {
            slide: PresentationSlide {
                id: Uuid::new_v4(),
                block_id: BlockRef::Direct(block_id),
            },
            index: self.slides.len(),
        }])
    }

    fn delete_child(&self, block_id: Uuid) -> Option<Vec<Self::Operation>> {
        let reference = BlockRef::Direct(block_id);
        Some(
            self.slides
                .iter()
                .filter(|slide| slide.block_id == reference)
                .map(|slide| PresentationOperation::Remove { slide_id: slide.id })
                .collect(),
        )
    }

    fn replace_child(&self, old: Uuid, new: Uuid) -> Option<Vec<Self::Operation>> {
        let old = BlockRef::Direct(old);
        Some(
            self.slides
                .iter()
                .filter(|slide| slide.block_id == old)
                .map(|slide| PresentationOperation::SetBlockId {
                    slide_id: slide.id,
                    block_id: BlockRef::Direct(new),
                })
                .collect(),
        )
    }
}

impl BlockHistory<Presentation> for PresentationHistory {
    type Action = PresentationHistoryAction;
    type Snapshot = Presentation;

    fn snapshot(block: &Presentation) -> Self::Snapshot {
        block.clone()
    }

    fn action(
        before: Presentation,
        _after: &Presentation,
        operations: &[PresentationOperation],
    ) -> Option<Self::Action> {
        let mut current = before;
        let mut changes = Vec::new();
        for operation in operations {
            match operation {
                PresentationOperation::Insert { slide, index }
                    if !current
                        .slides
                        .iter()
                        .any(|existing| existing.id == slide.id) =>
                {
                    let index = (*index).min(current.slides.len());
                    changes.push(PresentationHistoryChange::Insert {
                        slide: slide.clone(),
                        index,
                    });
                }
                PresentationOperation::Remove { slide_id } => {
                    if let Some(index) = current
                        .slides
                        .iter()
                        .position(|slide| slide.id == *slide_id)
                    {
                        changes.push(PresentationHistoryChange::Remove {
                            slide: current.slides[index].clone(),
                            index,
                        });
                    }
                }
                PresentationOperation::Move { slide_id, index } => {
                    if let Some(before) = current
                        .slides
                        .iter()
                        .position(|slide| slide.id == *slide_id)
                    {
                        let after = (*index).min(current.slides.len().saturating_sub(1));
                        if before != after {
                            changes.push(PresentationHistoryChange::Move {
                                slide_id: *slide_id,
                                before,
                                after,
                            });
                        }
                    }
                }
                PresentationOperation::SetBlockId { slide_id, block_id } => {
                    if let Some(slide) = current
                        .slides
                        .iter()
                        .find(|slide| slide.id == *slide_id && slide.block_id != *block_id)
                    {
                        changes.push(PresentationHistoryChange::SetBlockId {
                            slide_id: *slide_id,
                            before: slide.block_id,
                            after: *block_id,
                        });
                    }
                }
                PresentationOperation::Insert { .. } => {}
            }
            Presentation::apply_operation(&mut current, operation);
        }
        (!changes.is_empty()).then_some(PresentationHistoryAction { changes })
    }

    fn action_bytes(action: &Self::Action) -> usize {
        action.changes.len() * 64
    }

    fn operations(
        _current: &Presentation,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) -> Vec<PresentationOperation> {
        let changes: Box<dyn Iterator<Item = &PresentationHistoryChange> + '_> =
            if direction == HistoryDirection::Undo {
                Box::new(action.changes.iter().rev())
            } else {
                Box::new(action.changes.iter())
            };
        changes
            .map(|change| match (direction, change) {
                (HistoryDirection::Undo, PresentationHistoryChange::Insert { slide, .. }) => {
                    PresentationOperation::Remove { slide_id: slide.id }
                }
                (HistoryDirection::Redo, PresentationHistoryChange::Insert { slide, index }) => {
                    PresentationOperation::Insert {
                        slide: slide.clone(),
                        index: *index,
                    }
                }
                (HistoryDirection::Undo, PresentationHistoryChange::Remove { slide, index }) => {
                    PresentationOperation::Insert {
                        slide: slide.clone(),
                        index: *index,
                    }
                }
                (HistoryDirection::Redo, PresentationHistoryChange::Remove { slide, .. }) => {
                    PresentationOperation::Remove { slide_id: slide.id }
                }
                (
                    HistoryDirection::Undo,
                    PresentationHistoryChange::Move {
                        slide_id, before, ..
                    },
                ) => PresentationOperation::Move {
                    slide_id: *slide_id,
                    index: *before,
                },
                (
                    HistoryDirection::Redo,
                    PresentationHistoryChange::Move {
                        slide_id, after, ..
                    },
                ) => PresentationOperation::Move {
                    slide_id: *slide_id,
                    index: *after,
                },
                (
                    HistoryDirection::Undo,
                    PresentationHistoryChange::SetBlockId {
                        slide_id, before, ..
                    },
                ) => PresentationOperation::SetBlockId {
                    slide_id: *slide_id,
                    block_id: *before,
                },
                (
                    HistoryDirection::Redo,
                    PresentationHistoryChange::SetBlockId {
                        slide_id, after, ..
                    },
                ) => PresentationOperation::SetBlockId {
                    slide_id: *slide_id,
                    block_id: *after,
                },
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
