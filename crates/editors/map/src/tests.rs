use std::sync::Arc;

use block_client::blocks::map::{Map, MapRegion};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::MapApp;

mod a_new_map_shows_the_whole_world;
mod the_sidebar_captures_the_preview_region;

fn editor() -> (EditorTest<'static, MapApp>, BlockHandle<Map>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Map::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = MapApp::default();
    app.connect(host.clone(), client, block.id());
    let mut editor = EditorTest::viewport(app, host);
    editor.step();
    (editor, block)
}
