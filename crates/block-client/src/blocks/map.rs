use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use block::{Block, BlockHistory, HistoryDirection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block_ref::BlockRef;

const EDIT_BURST_DELAY: Duration = Duration::from_millis(750);
/// Latitude beyond which the Web Mercator projection used by the map tiles is
/// undefined.
pub const MAX_LATITUDE: f64 = 85.051_128_78;
/// Smallest span a preview region may cover, in degrees.
pub const MIN_REGION_SPAN: f64 = 0.000_01;

/// A geographic position in degrees.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct MapCoordinate {
    pub longitude: f64,
    pub latitude: f64,
}

impl MapCoordinate {
    pub const fn new(longitude: f64, latitude: f64) -> Self {
        Self {
            longitude,
            latitude,
        }
    }
}

/// The geographic rectangle a map shows when it is previewed, presented, or
/// first opened.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct MapRegion {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

impl MapRegion {
    pub const fn new(west: f64, south: f64, east: f64, north: f64) -> Self {
        Self {
            west,
            south,
            east,
            north,
        }
    }

    /// The whole world, which is what a map without a region shows.
    pub const WORLD: Self = Self::new(-180.0, -MAX_LATITUDE, 180.0, MAX_LATITUDE);

    pub fn center(self) -> MapCoordinate {
        MapCoordinate::new(
            (self.west + self.east) * 0.5,
            (self.south + self.north) * 0.5,
        )
    }
}

/// The color of a point of interest marker.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MapColor {
    #[default]
    Default,
    Rgb {
        red: u8,
        green: u8,
        blue: u8,
    },
}

/// A block placed on the map at a geographic position.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct MapPoint {
    pub id: Uuid,
    pub block_id: BlockRef,
    pub position: MapCoordinate,
    pub color: MapColor,
}

impl MapPoint {
    pub fn new(block_id: BlockRef, position: MapCoordinate) -> Self {
        Self {
            id: Uuid::new_v4(),
            block_id,
            position,
            color: MapColor::Default,
        }
    }
}

/// A world map rendered from OpenStreetMap vector tiles, with blocks pinned to
/// geographic positions.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Map {
    points: Vec<MapPoint>,
    preview_region: Option<MapRegion>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum MapOperation {
    AddPoint { point: MapPoint },
    UpdatePoints { points: Vec<MapPoint> },
    RemovePoints { ids: Vec<Uuid> },
    SetPreviewRegion { region: Option<MapRegion> },
}

pub struct MapHistory;

pub struct MapHistoryAction {
    kind: MapHistoryActionKind,
    recorded_at: Instant,
}

enum MapHistoryActionKind {
    Add(MapPoint),
    Remove(Vec<MapPoint>),
    Update {
        before: Vec<MapPoint>,
        after: Vec<MapPoint>,
    },
    PreviewRegion {
        before: Option<MapRegion>,
        after: Option<MapRegion>,
    },
}

impl Map {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn points(&self) -> &[MapPoint] {
        &self.points
    }

    pub fn point(&self, id: Uuid) -> Option<MapPoint> {
        self.points.iter().copied().find(|point| point.id == id)
    }

    pub fn preview_region(&self) -> Option<MapRegion> {
        self.preview_region
    }

    /// The region the map shows when it is previewed or first opened.
    pub fn displayed_region(&self) -> MapRegion {
        self.preview_region.unwrap_or(MapRegion::WORLD)
    }
}

impl Block for Map {
    type Operation = MapOperation;
    type History = MapHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6d61_7076_6965_7762_6c6f_636b_0000_0001);

    fn apply_operation(map: &mut Self, operation: &Self::Operation) {
        match operation {
            MapOperation::AddPoint { point } => {
                if map.points.iter().any(|existing| existing.id == point.id) {
                    return;
                }
                map.points.push(normalized_point(*point));
            }
            MapOperation::UpdatePoints { points } => {
                for update in points {
                    if let Some(point) = map.points.iter_mut().find(|point| point.id == update.id) {
                        *point = normalized_point(*update);
                    }
                }
            }
            MapOperation::RemovePoints { ids } => {
                let ids: HashSet<_> = ids.iter().copied().collect();
                map.points.retain(|point| !ids.contains(&point.id));
            }
            MapOperation::SetPreviewRegion { region } => {
                map.preview_region = region.map(normalized_region);
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        self.points
            .iter()
            .filter_map(|point| point.block_id.as_direct())
            .filter(|block_id| seen.insert(*block_id))
            .collect()
    }
}

fn normalized_point(mut point: MapPoint) -> MapPoint {
    point.position.longitude = clamp_finite(point.position.longitude, -180.0, 180.0);
    point.position.latitude = clamp_finite(point.position.latitude, -MAX_LATITUDE, MAX_LATITUDE);
    point
}

fn normalized_region(region: MapRegion) -> MapRegion {
    let west = clamp_finite(region.west, -180.0, 180.0);
    let east = clamp_finite(region.east, -180.0, 180.0);
    let south = clamp_finite(region.south, -MAX_LATITUDE, MAX_LATITUDE);
    let north = clamp_finite(region.north, -MAX_LATITUDE, MAX_LATITUDE);
    let (west, east) = ordered_span(west, east, -180.0, 180.0);
    let (south, north) = ordered_span(south, north, -MAX_LATITUDE, MAX_LATITUDE);
    MapRegion::new(west, south, east, north)
}

/// Orders one axis of a region and widens it to the minimum span without
/// leaving the projection limits.
fn ordered_span(low: f64, high: f64, limit_low: f64, limit_high: f64) -> (f64, f64) {
    let (mut low, mut high) = (low.min(high), low.max(high));
    if high - low >= MIN_REGION_SPAN {
        return (low, high);
    }
    let center = (low + high) * 0.5;
    low = center - MIN_REGION_SPAN * 0.5;
    high = center + MIN_REGION_SPAN * 0.5;
    if low < limit_low {
        (limit_low, limit_low + MIN_REGION_SPAN)
    } else if high > limit_high {
        (limit_high - MIN_REGION_SPAN, limit_high)
    } else {
        (low, high)
    }
}

fn clamp_finite(value: f64, low: f64, high: f64) -> f64 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        0.0
    }
}

impl BlockHistory<Map> for MapHistory {
    type Action = MapHistoryAction;
    type Snapshot = Map;

    fn snapshot(block: &Map) -> Self::Snapshot {
        block.clone()
    }

    fn action(before: Map, after: &Map, operations: &[MapOperation]) -> Option<Self::Action> {
        let operation = operations.last()?;
        let kind = match operation {
            MapOperation::AddPoint { point } => {
                let added = after.point(point.id)?;
                if before.point(point.id).is_some() {
                    return None;
                }
                MapHistoryActionKind::Add(added)
            }
            MapOperation::RemovePoints { ids } => {
                let removed = before
                    .points
                    .iter()
                    .copied()
                    .filter(|point| ids.contains(&point.id))
                    .collect::<Vec<_>>();
                if removed.is_empty() {
                    return None;
                }
                MapHistoryActionKind::Remove(removed)
            }
            MapOperation::UpdatePoints { points } => {
                let mut changed_before = Vec::new();
                let mut changed_after = Vec::new();
                for update in points {
                    let (Some(previous), Some(current)) =
                        (before.point(update.id), after.point(update.id))
                    else {
                        continue;
                    };
                    if previous != current {
                        changed_before.push(previous);
                        changed_after.push(current);
                    }
                }
                if changed_after.is_empty() {
                    return None;
                }
                MapHistoryActionKind::Update {
                    before: changed_before,
                    after: changed_after,
                }
            }
            MapOperation::SetPreviewRegion { .. } => {
                if before.preview_region == after.preview_region {
                    return None;
                }
                MapHistoryActionKind::PreviewRegion {
                    before: before.preview_region,
                    after: after.preview_region,
                }
            }
        };
        Some(MapHistoryAction {
            kind,
            recorded_at: Instant::now(),
        })
    }

    fn action_bytes(action: &Self::Action) -> usize {
        match &action.kind {
            MapHistoryActionKind::Add(_) => size_of::<MapPoint>(),
            MapHistoryActionKind::Remove(points) => points.len() * size_of::<MapPoint>(),
            MapHistoryActionKind::Update { before, .. } => before.len() * size_of::<MapPoint>() * 2,
            MapHistoryActionKind::PreviewRegion { .. } => size_of::<MapRegion>() * 2,
        }
    }

    fn merge(previous: &mut Self::Action, next: Self::Action) -> Result<(), Self::Action> {
        if next.recorded_at.duration_since(previous.recorded_at) > EDIT_BURST_DELAY {
            return Err(next);
        }
        let next_recorded_at = next.recorded_at;
        match (&mut previous.kind, &next.kind) {
            (
                MapHistoryActionKind::Update {
                    after: previous_after,
                    ..
                },
                MapHistoryActionKind::Update {
                    after: next_after, ..
                },
            ) => {
                let previous_ids = previous_after
                    .iter()
                    .map(|point| point.id)
                    .collect::<Vec<_>>();
                let next_ids = next_after.iter().map(|point| point.id).collect::<Vec<_>>();
                if previous_ids != next_ids {
                    return Err(next);
                }
                previous_after.clone_from(next_after);
            }
            (
                MapHistoryActionKind::PreviewRegion {
                    after: previous_after,
                    ..
                },
                MapHistoryActionKind::PreviewRegion {
                    after: next_after, ..
                },
            ) => *previous_after = *next_after,
            _ => return Err(next),
        }
        previous.recorded_at = next_recorded_at;
        Ok(())
    }

    fn operations(
        current: &Map,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) -> Vec<MapOperation> {
        let to_after = direction == HistoryDirection::Redo;
        match &action.kind {
            MapHistoryActionKind::Add(point) => {
                if to_after {
                    vec![MapOperation::AddPoint { point: *point }]
                } else {
                    vec![MapOperation::RemovePoints {
                        ids: vec![point.id],
                    }]
                }
            }
            MapHistoryActionKind::Remove(points) => {
                if to_after {
                    vec![MapOperation::RemovePoints {
                        ids: points.iter().map(|point| point.id).collect(),
                    }]
                } else {
                    points
                        .iter()
                        .map(|point| MapOperation::AddPoint { point: *point })
                        .collect()
                }
            }
            MapHistoryActionKind::Update { before, after } => {
                let (expected, desired) = if to_after {
                    (before, after)
                } else {
                    (after, before)
                };
                let points = expected
                    .iter()
                    .zip(desired)
                    .filter_map(|(expected, desired)| {
                        current
                            .point(expected.id)
                            .map(|current| rebase_point(current, *expected, *desired))
                    })
                    .collect::<Vec<_>>();
                (!points.is_empty())
                    .then_some(MapOperation::UpdatePoints { points })
                    .into_iter()
                    .collect()
            }
            MapHistoryActionKind::PreviewRegion { before, after } => {
                vec![MapOperation::SetPreviewRegion {
                    region: if to_after { *after } else { *before },
                }]
            }
        }
    }
}

/// Keeps fields a concurrent editor changed while restoring the fields this
/// history action owns.
fn rebase_point(current: MapPoint, expected: MapPoint, desired: MapPoint) -> MapPoint {
    let mut result = current;
    if result.position == expected.position {
        result.position = desired.position;
    }
    if result.color == expected.color {
        result.color = desired.color;
    }
    result
}

#[cfg(test)]
mod tests;
