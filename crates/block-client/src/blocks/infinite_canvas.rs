use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use block::{Block, BlockHistory, HistoryDirection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block_ref::BlockRef;

mod presence;

pub use presence::CanvasCursor;

const EDIT_BURST_DELAY: Duration = Duration::from_millis(750);

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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasPreviewRegion {
    pub center: CanvasPoint,
    pub size: CanvasPoint,
}

impl CanvasPreviewRegion {
    pub const fn new(center: CanvasPoint, size: CanvasPoint) -> Self {
        Self { center, size }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasEntityKind {
    Line,
    Rectangle,
    Text {
        text: String,
        text_style: CanvasTextStyle,
        placeholder: String,
    },
    Pen {
        points: Vec<CanvasPoint>,
    },
    Block {
        block_id: BlockRef,
    },
    DirectEditor {
        block_id: BlockRef,
        scale: f32,
    },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasTextWeight {
    #[default]
    Regular,
    Bold,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasTextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasTextStyle {
    pub font_size: f32,
    pub weight: CanvasTextWeight,
    pub alignment: CanvasTextAlign,
    pub line_height: f32,
    pub wrap: bool,
}

impl Default for CanvasTextStyle {
    fn default() -> Self {
        Self {
            font_size: 18.0,
            weight: CanvasTextWeight::Regular,
            alignment: CanvasTextAlign::Left,
            line_height: 1.2,
            wrap: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasColor {
    #[default]
    Auto,
    Rgba {
        red: u8,
        green: u8,
        blue: u8,
        alpha: u8,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasEntityStyle {
    pub foreground: CanvasColor,
    pub line_width: f32,
    pub dashed: bool,
    pub fill: Option<CanvasColor>,
    pub arrow_start: bool,
    pub arrow_end: bool,
    pub corner_radius: f32,
    pub opacity: f32,
}

impl Default for CanvasEntityStyle {
    fn default() -> Self {
        Self {
            foreground: CanvasColor::Auto,
            line_width: 2.0,
            dashed: false,
            fill: None,
            arrow_start: false,
            arrow_end: false,
            corner_radius: 0.0,
            opacity: 1.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasEntity {
    pub id: Uuid,
    pub transform: CanvasTransform,
    pub kind: CanvasEntityKind,
    pub style: CanvasEntityStyle,
    pub group_id: Option<Uuid>,
    pub locked: bool,
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
    ExactOrder {
        ids: Vec<Uuid>,
    },
    SetPreviewRegion {
        region: Option<CanvasPreviewRegion>,
    },
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct InfiniteCanvas {
    entities: Vec<CanvasEntity>,
    preview_region: Option<CanvasPreviewRegion>,
}

pub struct InfiniteCanvasHistory;

pub struct InfiniteCanvasHistoryAction {
    kind: InfiniteCanvasHistoryActionKind,
    recorded_at: Instant,
}

enum InfiniteCanvasHistoryActionKind {
    Add(CanvasEntity),
    Remove {
        entities: Vec<CanvasEntity>,
        order: Vec<Uuid>,
    },
    Update {
        before: Vec<CanvasEntity>,
        after: Vec<CanvasEntity>,
    },
    Order {
        before: Vec<Uuid>,
        after: Vec<Uuid>,
    },
    PreviewRegion {
        before: Option<CanvasPreviewRegion>,
        after: Option<CanvasPreviewRegion>,
    },
}

impl InfiniteCanvas {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entities(&self) -> &[CanvasEntity] {
        &self.entities
    }

    pub fn preview_region(&self) -> Option<CanvasPreviewRegion> {
        self.preview_region
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
    type History = InfiniteCanvasHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x696e_6669_6e69_7465_2d63_616e_7661_7301);

    fn apply_operation(canvas: &mut Self, operation: &Self::Operation) {
        match operation {
            InfiniteCanvasOperation::Add { entity } => {
                let entity = normalized_entity(entity.clone());
                if !canvas
                    .entities
                    .iter()
                    .any(|existing| existing.id == entity.id)
                {
                    canvas.entities.push(entity);
                }
            }
            InfiniteCanvasOperation::Update { entities } => {
                for update in entities {
                    if let Some(entity) = canvas
                        .entities
                        .iter_mut()
                        .find(|entity| entity.id == update.id)
                    {
                        *entity = normalized_entity(update.clone());
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
            InfiniteCanvasOperation::ExactOrder { ids } => {
                let mut ordered = canvas.entities.clone();
                let mut ranked = canvas
                    .entities
                    .iter()
                    .filter(|entity| ids.contains(&entity.id))
                    .cloned()
                    .collect::<Vec<_>>();
                ranked.sort_by_key(|entity| {
                    ids.iter()
                        .position(|id| *id == entity.id)
                        .unwrap_or(usize::MAX)
                });
                let mut ranked = ranked.into_iter();
                for (index, entity) in canvas.entities.iter().enumerate() {
                    if ids.contains(&entity.id) {
                        ordered[index] = ranked.next().unwrap();
                    }
                }
                canvas.entities = ordered;
            }
            InfiniteCanvasOperation::SetPreviewRegion { region } => {
                canvas.preview_region = region.map(normalized_preview_region);
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        let mut references: Vec<_> = self
            .entities
            .iter()
            .filter_map(|entity| match entity.kind {
                CanvasEntityKind::Block { block_id }
                | CanvasEntityKind::DirectEditor { block_id, .. } => block_id.as_direct(),
                _ => None,
            })
            .collect();
        let mut seen = HashSet::new();
        references.retain(|reference| seen.insert(*reference));
        references
    }
}

fn normalized_entity(mut entity: CanvasEntity) -> CanvasEntity {
    if let CanvasEntityKind::DirectEditor { scale, .. } = &mut entity.kind {
        entity.transform.rotation = 0.0;
        if !scale.is_finite() || *scale <= 0.0 {
            *scale = 1.0;
        }
    }
    entity
}

fn normalized_preview_region(mut region: CanvasPreviewRegion) -> CanvasPreviewRegion {
    if !region.center.x.is_finite() {
        region.center.x = 0.0;
    }
    if !region.center.y.is_finite() {
        region.center.y = 0.0;
    }
    if !region.size.x.is_finite() {
        region.size.x = 100.0;
    }
    if !region.size.y.is_finite() {
        region.size.y = 100.0;
    }
    region.size.x = region.size.x.abs().max(1.0);
    region.size.y = region.size.y.abs().max(1.0);
    region
}

impl BlockHistory<InfiniteCanvas> for InfiniteCanvasHistory {
    type Action = InfiniteCanvasHistoryAction;
    type Snapshot = InfiniteCanvas;

    fn snapshot(block: &InfiniteCanvas) -> Self::Snapshot {
        block.clone()
    }

    fn action(
        before: InfiniteCanvas,
        after: &InfiniteCanvas,
        operations: &[InfiniteCanvasOperation],
    ) -> Option<Self::Action> {
        let operation = operations.last()?;
        let kind = match operation {
            InfiniteCanvasOperation::Add { entity } => {
                InfiniteCanvasHistoryActionKind::Add(entity.clone())
            }
            InfiniteCanvasOperation::Remove { ids } => {
                let entities = before
                    .entities
                    .iter()
                    .filter(|entity| ids.contains(&entity.id))
                    .cloned()
                    .collect::<Vec<_>>();
                if entities.is_empty() {
                    return None;
                }
                InfiniteCanvasHistoryActionKind::Remove {
                    entities,
                    order: before.entities.iter().map(|entity| entity.id).collect(),
                }
            }
            InfiniteCanvasOperation::Update { entities } => {
                let mut changed_before = Vec::new();
                let mut changed_after = Vec::new();
                for updated in entities {
                    let Some(previous) = before
                        .entities
                        .iter()
                        .find(|entity| entity.id == updated.id)
                    else {
                        continue;
                    };
                    let Some(current) =
                        after.entities.iter().find(|entity| entity.id == updated.id)
                    else {
                        continue;
                    };
                    if previous != current {
                        changed_before.push(previous.clone());
                        changed_after.push(current.clone());
                    }
                }
                if changed_after.is_empty() {
                    return None;
                }
                InfiniteCanvasHistoryActionKind::Update {
                    before: changed_before,
                    after: changed_after,
                }
            }
            InfiniteCanvasOperation::Reorder { .. }
            | InfiniteCanvasOperation::ExactOrder { .. } => {
                let before = before
                    .entities
                    .iter()
                    .map(|entity| entity.id)
                    .collect::<Vec<_>>();
                let after = after
                    .entities
                    .iter()
                    .map(|entity| entity.id)
                    .collect::<Vec<_>>();
                if before == after {
                    return None;
                }
                InfiniteCanvasHistoryActionKind::Order { before, after }
            }
            InfiniteCanvasOperation::SetPreviewRegion { .. } => {
                if before.preview_region == after.preview_region {
                    return None;
                }
                InfiniteCanvasHistoryActionKind::PreviewRegion {
                    before: before.preview_region,
                    after: after.preview_region,
                }
            }
        };
        Some(InfiniteCanvasHistoryAction {
            kind,
            recorded_at: Instant::now(),
        })
    }

    fn action_bytes(action: &Self::Action) -> usize {
        match &action.kind {
            InfiniteCanvasHistoryActionKind::Add(_) => 512,
            InfiniteCanvasHistoryActionKind::Remove { entities, .. }
            | InfiniteCanvasHistoryActionKind::Update {
                before: entities, ..
            } => entities.len() * 1024,
            InfiniteCanvasHistoryActionKind::Order { before, .. } => {
                before.len() * size_of::<Uuid>() * 2
            }
            InfiniteCanvasHistoryActionKind::PreviewRegion { .. } => {
                size_of::<CanvasPreviewRegion>() * 2
            }
        }
    }

    fn merge(previous: &mut Self::Action, next: Self::Action) -> Result<(), Self::Action> {
        if next.recorded_at.duration_since(previous.recorded_at) > EDIT_BURST_DELAY {
            return Err(next);
        }
        let next_recorded_at = next.recorded_at;
        match (&mut previous.kind, &next.kind) {
            (
                InfiniteCanvasHistoryActionKind::Update {
                    before: previous_before,
                    after: previous_after,
                },
                InfiniteCanvasHistoryActionKind::Update {
                    before: next_before,
                    after: next_after,
                },
            ) => {
                let previous_ids = previous_after
                    .iter()
                    .map(|entity| entity.id)
                    .collect::<Vec<_>>();
                let next_ids = next_after
                    .iter()
                    .map(|entity| entity.id)
                    .collect::<Vec<_>>();
                if previous_ids != next_ids {
                    return Err(next);
                }
                if previous_before.is_empty() {
                    previous_before.clone_from(next_before);
                }
                previous_after.clone_from(next_after);
            }
            (
                InfiniteCanvasHistoryActionKind::PreviewRegion {
                    after: previous_after,
                    ..
                },
                InfiniteCanvasHistoryActionKind::PreviewRegion {
                    after: next_after, ..
                },
            ) => previous_after.clone_from(next_after),
            _ => return Err(next),
        }
        previous.recorded_at = next_recorded_at;
        Ok(())
    }

    fn operations(
        current: &InfiniteCanvas,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) -> Vec<InfiniteCanvasOperation> {
        let to_after = direction == HistoryDirection::Redo;
        match &action.kind {
            InfiniteCanvasHistoryActionKind::Add(entity) => {
                if to_after {
                    vec![InfiniteCanvasOperation::Add {
                        entity: entity.clone(),
                    }]
                } else {
                    vec![InfiniteCanvasOperation::Remove {
                        ids: vec![entity.id],
                    }]
                }
            }
            InfiniteCanvasHistoryActionKind::Remove { entities, order } => {
                if to_after {
                    vec![InfiniteCanvasOperation::Remove {
                        ids: entities.iter().map(|entity| entity.id).collect(),
                    }]
                } else {
                    let mut operations = entities
                        .iter()
                        .cloned()
                        .map(|entity| InfiniteCanvasOperation::Add { entity })
                        .collect::<Vec<_>>();
                    operations.push(InfiniteCanvasOperation::ExactOrder { ids: order.clone() });
                    operations
                }
            }
            InfiniteCanvasHistoryActionKind::Update { before, after } => {
                let (expected, desired) = if to_after {
                    (before, after)
                } else {
                    (after, before)
                };
                let entities = expected
                    .iter()
                    .zip(desired)
                    .filter_map(|(expected, desired)| {
                        current
                            .entities
                            .iter()
                            .find(|entity| entity.id == expected.id)
                            .map(|current| rebase_entity(current, expected, desired))
                    })
                    .collect::<Vec<_>>();
                (!entities.is_empty())
                    .then_some(InfiniteCanvasOperation::Update { entities })
                    .into_iter()
                    .collect()
            }
            InfiniteCanvasHistoryActionKind::Order { before, after } => {
                vec![InfiniteCanvasOperation::ExactOrder {
                    ids: if to_after {
                        after.clone()
                    } else {
                        before.clone()
                    },
                }]
            }
            InfiniteCanvasHistoryActionKind::PreviewRegion { before, after } => {
                vec![InfiniteCanvasOperation::SetPreviewRegion {
                    region: if to_after { *after } else { *before },
                }]
            }
        }
    }
}

fn rebase_entity(
    current: &CanvasEntity,
    expected: &CanvasEntity,
    desired: &CanvasEntity,
) -> CanvasEntity {
    let mut result = current.clone();
    replace_if_unchanged(
        &mut result.transform.center.x,
        expected.transform.center.x,
        desired.transform.center.x,
    );
    replace_if_unchanged(
        &mut result.transform.center.y,
        expected.transform.center.y,
        desired.transform.center.y,
    );
    replace_if_unchanged(
        &mut result.transform.size.x,
        expected.transform.size.x,
        desired.transform.size.x,
    );
    replace_if_unchanged(
        &mut result.transform.size.y,
        expected.transform.size.y,
        desired.transform.size.y,
    );
    replace_if_unchanged(
        &mut result.transform.rotation,
        expected.transform.rotation,
        desired.transform.rotation,
    );
    replace_if_unchanged(
        &mut result.kind,
        expected.kind.clone(),
        desired.kind.clone(),
    );
    replace_if_unchanged(&mut result.group_id, expected.group_id, desired.group_id);
    replace_if_unchanged(&mut result.locked, expected.locked, desired.locked);
    replace_if_unchanged(
        &mut result.style.foreground,
        expected.style.foreground,
        desired.style.foreground,
    );
    replace_if_unchanged(
        &mut result.style.line_width,
        expected.style.line_width,
        desired.style.line_width,
    );
    replace_if_unchanged(
        &mut result.style.dashed,
        expected.style.dashed,
        desired.style.dashed,
    );
    replace_if_unchanged(
        &mut result.style.fill,
        expected.style.fill,
        desired.style.fill,
    );
    replace_if_unchanged(
        &mut result.style.arrow_start,
        expected.style.arrow_start,
        desired.style.arrow_start,
    );
    replace_if_unchanged(
        &mut result.style.arrow_end,
        expected.style.arrow_end,
        desired.style.arrow_end,
    );
    replace_if_unchanged(
        &mut result.style.corner_radius,
        expected.style.corner_radius,
        desired.style.corner_radius,
    );
    replace_if_unchanged(
        &mut result.style.opacity,
        expected.style.opacity,
        desired.style.opacity,
    );
    result
}

fn replace_if_unchanged<T: PartialEq>(current: &mut T, expected: T, desired: T) {
    if *current == expected {
        *current = desired;
    }
}

#[cfg(test)]
mod tests;
