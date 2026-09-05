use std::sync::Arc;

use block_editor_plugin::block_ui::{BlockTypeEntry, ChildEdits};

use super::*;

mod access_mode_icon_marks_limited_access;
mod deleting_needs_a_container_that_can_delete_children;
mod unlinking_needs_a_container_that_can_replace_a_child;

const CONTAINER: Uuid = Uuid::from_u128(1);
const LISTED: Uuid = Uuid::from_u128(2);

fn catalog(edits: ChildEdits) -> BlockCatalog {
    BlockCatalog::new([(
        LISTED,
        BlockTypeEntry {
            display_name: "Listed".to_owned(),
            icon: None,
            child_edits: edits,
        },
    )])
}

fn app() -> FileTreeApp {
    let mut app = FileTreeApp::default();
    app.block_types.insert(CONTAINER, LISTED);
    app
}

fn client() -> Arc<BlockClient> {
    Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()))
}
