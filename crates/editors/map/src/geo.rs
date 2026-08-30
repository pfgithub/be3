use std::f64::consts::PI;

use block_client::blocks::map::{MapCoordinate, MapRegion, MAX_LATITUDE};
use block_editor_plugin::egui::{Pos2, Rect, Vec2};

#[derive(Clone, Copy)]
pub(crate) struct MapView {
    origin: [f64; 2],
    size: f64,
}

impl MapView {
    pub(crate) fn from_world_rect(world: Rect) -> Self {
        Self {
            origin: [f64::from(world.left()), f64::from(world.top())],
            size: f64::from(world.width().max(1.0)),
        }
    }

    pub(crate) fn covering(region: MapRegion, rect: Rect, max_size: f64) -> Self {
        let normalized = normalized_rect(region);
        let size = (f64::from(rect.width()) / normalized.width())
            .max(f64::from(rect.height()) / normalized.height())
            .clamp(1.0, max_size);
        let center = normalized.center();
        Self {
            origin: [
                f64::from(rect.center().x) - center[0] * size,
                f64::from(rect.center().y) - center[1] * size,
            ],
            size,
        }
    }

    pub(crate) fn world_rect(self) -> Rect {
        Rect::from_min_size(
            Pos2::new(self.origin[0] as f32, self.origin[1] as f32),
            Vec2::splat(self.size as f32),
        )
    }

    pub(crate) fn position(self, coordinate: MapCoordinate) -> Pos2 {
        Pos2::new(
            (self.origin[0] + normalized_x(coordinate.longitude) * self.size) as f32,
            (self.origin[1] + normalized_y(coordinate.latitude) * self.size) as f32,
        )
    }

    pub(crate) fn coordinate(self, position: Pos2) -> MapCoordinate {
        MapCoordinate::new(
            longitude_from_normalized((f64::from(position.x) - self.origin[0]) / self.size),
            latitude_from_normalized((f64::from(position.y) - self.origin[1]) / self.size),
        )
    }

    pub(crate) fn region_rect(self, region: MapRegion) -> Rect {
        Rect::from_min_max(
            self.position(MapCoordinate::new(region.west, region.north)),
            self.position(MapCoordinate::new(region.east, region.south)),
        )
    }

    pub(crate) fn region(self, rect: Rect) -> MapRegion {
        let top_left = self.coordinate(rect.left_top());
        let bottom_right = self.coordinate(rect.right_bottom());
        MapRegion::new(
            top_left.longitude,
            bottom_right.latitude,
            bottom_right.longitude,
            top_left.latitude,
        )
    }
}

struct NormalizedRect {
    min: [f64; 2],
    max: [f64; 2],
}

impl NormalizedRect {
    fn width(&self) -> f64 {
        (self.max[0] - self.min[0]).max(f64::EPSILON)
    }

    fn height(&self) -> f64 {
        (self.max[1] - self.min[1]).max(f64::EPSILON)
    }

    fn center(&self) -> [f64; 2] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
        ]
    }
}

fn normalized_rect(region: MapRegion) -> NormalizedRect {
    NormalizedRect {
        min: [normalized_x(region.west), normalized_y(region.north)],
        max: [normalized_x(region.east), normalized_y(region.south)],
    }
}

pub(crate) fn region_aspect_ratio(region: MapRegion) -> f32 {
    let normalized = normalized_rect(region);
    (normalized.width() / normalized.height()) as f32
}

fn normalized_x(longitude: f64) -> f64 {
    (longitude.clamp(-180.0, 180.0) + 180.0) / 360.0
}

fn longitude_from_normalized(x: f64) -> f64 {
    (x * 360.0 - 180.0).clamp(-180.0, 180.0)
}

fn normalized_y(latitude: f64) -> f64 {
    let latitude = latitude.clamp(-MAX_LATITUDE, MAX_LATITUDE).to_radians();
    (1.0 - (latitude.tan() + 1.0 / latitude.cos()).ln() / PI) * 0.5
}

fn latitude_from_normalized(y: f64) -> f64 {
    let projected = PI * (1.0 - 2.0 * y);
    projected
        .sinh()
        .atan()
        .to_degrees()
        .clamp(-MAX_LATITUDE, MAX_LATITUDE)
}
