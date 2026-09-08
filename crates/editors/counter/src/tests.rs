use std::sync::Arc;

use block_client::blocks::counter::Counter;
use block_client::{BlockClient, BlockHandle};
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

fn shown(editor: &mut BeuiTest<CounterApp>) -> String {
    let app = editor.app();
    let ui = app.ui().expect("the ui is not open yet");
    let value = ui
        .document()
        .find_test_id("counter.value")
        .expect("the ui has no counter.value node");
    ui.document()
        .node_detail(value)
        .expect("the value node has no text")
}
