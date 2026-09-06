use std::sync::Arc;

use beui::NodeId;
use block_client::blocks::counter::Counter;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{egui, App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::CounterApp;

mod clicking_the_plus_button_counts_up_on_the_block;
mod resetting_puts_the_block_back_to_zero;
mod the_counter_shows_what_the_block_holds;

fn editor() -> (EditorTest<'static, CounterApp>, BlockHandle<Counter>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Counter::default());
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = CounterApp::default();
    app.connect(host, client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block)
}

fn click(editor: &mut EditorTest<'_, CounterApp>, node: impl Fn(&CounterApp) -> NodeId) {
    let app = editor.app();
    let target = node(app);
    let rect = app
        .demo()
        .and_then(|demo| demo.document().node_rect(target))
        .expect("the demo has not laid that node out");
    let center = rect.center();
    editor.click_at(egui::pos2(center.x, center.y));
    editor.run();
}

fn shown(editor: &mut EditorTest<'_, CounterApp>) -> String {
    let app = editor.app();
    let demo = app.demo().expect("the demo is not open yet");
    demo.document()
        .node_detail(demo.value_node())
        .expect("the value node has no text")
}
