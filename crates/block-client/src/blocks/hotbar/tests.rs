use uuid::Uuid;

use super::{Hotbar, HotbarOperation, HotbarSlot};
use crate::BlockClient;

fn component(name: &str, compiled: Uuid) -> HotbarSlot {
    HotbarSlot::Component {
        name: name.to_owned(),
        compiled,
    }
}

mod hotbar_history_restores_the_previous_layout;
mod hotbar_references_components_nested_in_folders;
