use std::collections::HashSet;

use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct CanvasPoint {
    pub x: f32,
    pub y: f32,
}

impl CanvasPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasTransform {
    pub center: CanvasPoint,
    pub size: CanvasPoint,
    pub rotation: f32,
}

impl CanvasTransform {
    pub const fn new(center: CanvasPoint, size: CanvasPoint, rotation: f32) -> Self {
        Self {
            center,
            size,
            rotation,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasEntityKind {
    Line,
    Rectangle,
    Text { text: String },
    Pen { points: Vec<CanvasPoint> },
    Block { block_id: Uuid },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasEntity {
    pub id: Uuid,
    pub transform: CanvasTransform,
    pub kind: CanvasEntityKind,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasLayerMove {
    BringToFront,
    ForwardOne,
    BackOne,
    SendToBack,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum InfiniteCanvasOperation {
    Add {
        entity: CanvasEntity,
    },
    Update {
        entities: Vec<CanvasEntity>,
    },
    Remove {
        ids: Vec<Uuid>,
    },
    Reorder {
        ids: Vec<Uuid>,
        movement: CanvasLayerMove,
    },
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct InfiniteCanvas {
    entities: Vec<CanvasEntity>,
}

impl InfiniteCanvas {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entities(&self) -> &[CanvasEntity] {
        &self.entities
    }

    fn reorder(&mut self, ids: &[Uuid], movement: CanvasLayerMove) {
        let selected: HashSet<_> = ids.iter().copied().collect();
        match movement {
            CanvasLayerMove::BringToFront => {
                let (mut back, front): (Vec<_>, Vec<_>) = self
                    .entities
                    .drain(..)
                    .partition(|entity| !selected.contains(&entity.id));
                back.extend(front);
                self.entities = back;
            }
            CanvasLayerMove::ForwardOne => {
                for index in (0..self.entities.len().saturating_sub(1)).rev() {
                    if selected.contains(&self.entities[index].id)
                        && !selected.contains(&self.entities[index + 1].id)
                    {
                        self.entities.swap(index, index + 1);
                    }
                }
            }
            CanvasLayerMove::BackOne => {
                for index in 1..self.entities.len() {
                    if selected.contains(&self.entities[index].id)
                        && !selected.contains(&self.entities[index - 1].id)
                    {
                        self.entities.swap(index - 1, index);
                    }
                }
            }
            CanvasLayerMove::SendToBack => {
                let (mut back, front): (Vec<_>, Vec<_>) = self
                    .entities
                    .drain(..)
                    .partition(|entity| selected.contains(&entity.id));
                back.extend(front);
                self.entities = back;
            }
        }
    }
}

impl Block for InfiniteCanvas {
    type Operation = InfiniteCanvasOperation;

    const TYPE_ID: Uuid = Uuid::from_u128(0x696e_6669_6e69_7465_2d63_616e_7661_7301);

    fn apply_operation(canvas: &mut Self, operation: &Self::Operation) {
        match operation {
            InfiniteCanvasOperation::Add { entity } => {
                if !canvas
                    .entities
                    .iter()
                    .any(|existing| existing.id == entity.id)
                {
                    canvas.entities.push(entity.clone());
                }
            }
            InfiniteCanvasOperation::Update { entities } => {
                for update in entities {
                    if let Some(entity) = canvas
                        .entities
                        .iter_mut()
                        .find(|entity| entity.id == update.id)
                    {
                        *entity = update.clone();
                    }
                }
            }
            InfiniteCanvasOperation::Remove { ids } => {
                let ids: HashSet<_> = ids.iter().copied().collect();
                canvas.entities.retain(|entity| !ids.contains(&entity.id));
            }
            InfiniteCanvasOperation::Reorder { ids, movement } => {
                canvas.reorder(ids, *movement);
            }
        }
    }

    fn implicit_name(&self) -> String {
        "Canvas".into()
    }

    fn references(&self) -> Vec<Uuid> {
        let mut references: Vec<_> = self
            .entities
            .iter()
            .filter_map(|entity| match entity.kind {
                CanvasEntityKind::Block { block_id } => Some(block_id),
                _ => None,
            })
            .collect();
        let mut seen = HashSet::new();
        references.retain(|reference| seen.insert(*reference));
        references
    }
}
