use std::collections::{BTreeMap, HashSet};

use super::inspector::{attach_component, remove_component, set_component_value};
use super::*;

fn entity(id: Uuid) -> CanvasEntity {
    CanvasEntity {
        id,
        transform: CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(10.0, 10.0), 0.0),
        kind: CanvasEntityKind::Rectangle,
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
        components: Vec::new(),
    }
}

mod attaching_component_fills_only_missing_selected_entities;
mod removing_component_deletes_its_values_from_all_selected_entities;
mod setting_component_value_writes_the_same_value_to_all_selected_entities;
