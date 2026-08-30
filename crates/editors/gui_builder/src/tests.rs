use std::sync::Arc;

use block_client::blocks::gui_builder::GuiBuilder;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::GuiBuilderApp;

mod adding_a_heading_puts_it_on_the_canvas;
mod resizing_the_editor_stores_the_canvas_size;

fn editor() -> (EditorTest<'static, GuiBuilderApp>, BlockHandle<GuiBuilder>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(GuiBuilder::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = GuiBuilderApp::default();
    app.connect(host, client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block)
}
