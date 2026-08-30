use std::sync::Arc;

use block_client::blocks::logic_game::LogicGame;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::LogicGameApp;

mod expanding_a_level_shows_its_goal;

fn editor() -> (EditorTest<'static, LogicGameApp>, BlockHandle<LogicGame>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(LogicGame::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = LogicGameApp::default();
    app.connect(host, client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block)
}
