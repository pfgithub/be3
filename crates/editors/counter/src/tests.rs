use std::sync::Arc;

use block_client::blocks::counter::Counter;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::App as _;
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::demo::CounterApp;

mod a_step_of_five_counts_five_at_a_time;

fn editor() -> (EditorTest<'static, CounterApp>, BlockHandle<Counter>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Counter::default());
    let mut app = CounterApp::default();
    app.connect(Default::default(), client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block)
}
