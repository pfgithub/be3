use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MIN_ZOOM: f64 = 0.0;
pub const MAX_ZOOM: f64 = 18.0;
/// Latitude bound of the Web Mercator projection.
pub const MAX_LATITUDE: f64 = 85.05112877980659;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct MapView {
    pub longitude: f64,
    pub latitude: f64,
    pub zoom: f64,
}

impl MapView {
    pub fn world() -> Self {
        Self {
            longitude: 0.0,
            latitude: 0.0,
            zoom: 1.0,
        }
    }

    pub fn clamped(self) -> Self {
        let longitude = if self.longitude.is_finite() {
            (self.longitude + 180.0).rem_euclid(360.0) - 180.0
        } else {
            0.0
        };
        let latitude = if self.latitude.is_finite() {
            self.latitude.clamp(-MAX_LATITUDE, MAX_LATITUDE)
        } else {
            0.0
        };
        let zoom = if self.zoom.is_finite() {
            self.zoom.clamp(MIN_ZOOM, MAX_ZOOM)
        } else {
            MIN_ZOOM
        };
        Self {
            longitude,
            latitude,
            zoom,
        }
    }
}

impl Default for MapView {
    fn default() -> Self {
        Self::world()
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Map {
    view: MapView,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum MapOperation {
    SetView { view: MapView },
}

impl Map {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn view(&self) -> MapView {
        self.view
    }
}

impl Block for Map {
    type Operation = MapOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6d61_7076_6965_7762_6c6f_636b_0000_0001);

    fn apply_operation(map: &mut Self, operation: &Self::Operation) {
        match operation {
            MapOperation::SetView { view } => map.view = view.clamped(),
        }
    }

    fn implicit_name(&self) -> String {
        "Map".into()
    }
}

#[cfg(test)]
mod tests;
