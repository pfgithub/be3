use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A world map rendered from OpenStreetMap vector tiles. The block itself
/// holds no data; the view is purely local editor state.
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Map {}

#[derive(Clone, Deserialize, Serialize)]
pub enum MapOperation {}

impl Map {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Block for Map {
    type Operation = MapOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6d61_7076_6965_7762_6c6f_636b_0000_0001);

    fn apply_operation(_map: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }

    fn implicit_name(&self) -> String {
        "Map".into()
    }
}
