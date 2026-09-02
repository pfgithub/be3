use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use block_client::BlockClient;
use block_editor_plugin::App as _;
use block_ui_test::EditorTest;

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

const ACCOUNT_ID: Uuid = Uuid::from_u128(0x11ac_c001_0000_4000_8000_0000_0000_0002);
const WORKSPACE_ID: Uuid = Uuid::from_u128(0x7a20_a314_e4aa_4ca7_b7ae_d68c_3249_0d9e);

fn editor(
    entities: &[CanvasEntity],
) -> (EditorTest<'static, CanvasApp>, BlockHandle<InfiniteCanvas>) {
    let client = Arc::new(BlockClient::new(ACCOUNT_ID, WORKSPACE_ID));
    let block = client.create_block(InfiniteCanvas::new());
    for entity in entities {
        block.operate(InfiniteCanvasOperation::Add {
            entity: entity.clone(),
        });
    }
    let mut app = CanvasApp::default();
    app.connect(Default::default(), client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block)
}

fn entities(block: &BlockHandle<InfiniteCanvas>) -> Vec<CanvasEntity> {
    block
        .read()
        .expect("the canvas is loaded")
        .entities()
        .to_vec()
}

mod attaching_component_fills_only_missing_selected_entities;
mod removing_component_deletes_its_values_from_all_selected_entities;
mod replacing_a_referenced_block_rewrites_the_entity;
mod resizing_an_editor_that_cannot_resize_scales_it;
mod setting_component_value_writes_the_same_value_to_all_selected_entities;
mod the_intrinsic_size_follows_the_preview_region;
