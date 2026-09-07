use std::sync::Arc;

use block_client::blocks::counter::Counter;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::beui::{pos2, NodeId};
use block_editor_plugin::{BeuiApp as _, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::CounterApp;

mod clicking_the_plus_button_counts_up_on_the_block;
mod resetting_puts_the_block_back_to_zero;
mod the_counter_shows_what_the_block_holds;

fn editor() -> (BeuiTest<CounterApp>, BlockHandle<Counter>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Counter::default());
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = CounterApp::default();
    app.connect(host, client, block.id());
    (BeuiTest::new(app), block)
}

fn click(editor: &mut BeuiTest<CounterApp>, node: impl Fn(&CounterApp) -> NodeId) {
    let app = editor.app();
    let target = node(app);
    let rect = app
        .ui()
        .and_then(|ui| ui.document().node_rect(target))
        .expect("the ui has not laid that node out");
    let center = rect.center();
    editor.click_at(pos2(center.x, center.y));
    editor.run();
}

fn shown(editor: &mut BeuiTest<CounterApp>) -> String {
    let app = editor.app();
    let ui = app.ui().expect("the ui is not open yet");
    ui.document()
        .node_detail(ui.value_node())
        .expect("the value node has no text")
}
