use std::sync::Arc;

use block_client::blocks::deterministic_game::DeterministicGame;
use block_client::BlockClient;
use block_editor_plugin::{App as _, AssetResult, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::DeterministicGameApp;
use crate::catalog::Catalog;

mod a_game_the_app_does_not_have_says_so;
mod an_unreadable_games_index_is_reported;

const ACCOUNT: Uuid = Uuid::from_u128(0x6465_742d_7465_7374_2d61_6363_6f75_6e74);
const WORKSPACE: Uuid = Uuid::from_u128(0x6465_742d_7465_7374_2d77_6f72_6b73_7061);

fn editor(game: &str) -> (EditorTest<'static, DeterministicGameApp>, EditorHost) {
    let client = Arc::new(BlockClient::new(ACCOUNT, WORKSPACE));
    let block = client.create_block(DeterministicGame::new(game.to_owned(), game.to_owned()));
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(ACCOUNT);
    let mut app = DeterministicGameApp::default();
    app.connect(host.clone(), client, block.id());
    let mut editor = EditorTest::new(app);
    editor.step();
    editor.step();
    (editor, host)
}

fn answer(host: &EditorHost, result: AssetResult) {
    let reads = host.take_asset_reads();
    assert_eq!(reads.len(), 1, "one asset was asked for");
    host.set_asset(reads[0].0, result);
}
