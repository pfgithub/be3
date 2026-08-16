use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 3.0;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct UiSettings {
    zoom: f32,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self { zoom: 1.0 }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum UiSettingsOperation {
    SetZoom { zoom: f32 },
}

impl UiSettings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }
}

impl Block for UiSettings {
    type Operation = UiSettingsOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7569_2d73_6574_7469_6e67_732d_626c_6b31);

    fn apply_operation(settings: &mut Self, operation: &Self::Operation) {
        match operation {
            UiSettingsOperation::SetZoom { zoom } => {
                settings.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
            }
        }
    }
}

#[cfg(test)]
mod tests;
