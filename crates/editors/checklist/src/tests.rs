use std::sync::Arc;

use block_client::blocks::checklist::{Checklist, ChecklistOperation};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::App as _;
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::ChecklistApp;

mod adding_an_item_puts_it_on_the_list;
mod filtering_to_open_hides_the_items_that_are_done;

fn editor(items: &[(&str, bool)]) -> (EditorTest<'static, ChecklistApp>, BlockHandle<Checklist>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Checklist::default());
    for (text, done) in items {
        block.operate(ChecklistOperation::Add {
            text: (*text).to_owned(),
        });
        if *done {
            let index = block.read().unwrap().items().len() as u32 - 1;
            block.operate(ChecklistOperation::SetDone { index, done: true });
        }
    }

    let mut app = ChecklistApp::default();
    app.connect(Default::default(), client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block)
}

fn items(block: &BlockHandle<Checklist>) -> Vec<(String, bool)> {
    block
        .read()
        .unwrap()
        .items()
        .iter()
        .map(|item| (item.text.clone(), item.done))
        .collect()
}
