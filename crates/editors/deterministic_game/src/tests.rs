use std::sync::Arc;

use block::Block as _;
use block_client::blocks::deterministic_game::DeterministicGame;
use block_client::blocks::game_module::GameModule;
use block_client::BlockClient;
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::{module_filter, DeterministicGameApp};

mod a_module_that_is_not_a_game_is_reported;
mod the_picker_asks_only_for_game_modules;

const ACCOUNT: Uuid = Uuid::from_u128(0x6465_742d_7465_7374_2d61_6363_6f75_6e74);
const WORKSPACE: Uuid = Uuid::from_u128(0x6465_742d_7465_7374_2d77_6f72_6b73_7061);

fn editor(module: Vec<u8>) -> EditorTest<'static, DeterministicGameApp> {
    let client = Arc::new(BlockClient::new(ACCOUNT, WORKSPACE));
    let module = client.create_block(GameModule::new("game.wasm", module));
    let block = client.create_block(DeterministicGame::new(module.id()));
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(ACCOUNT);
    let mut app = DeterministicGameApp::default();
    app.connect(host, client, block.id());
    let mut editor = EditorTest::new(app);
    editor.step();
    editor.step();
    editor
}
