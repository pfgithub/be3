use std::sync::Arc;

use block::Block;
use block_client::blocks::settings::Settings;
use block_client::blocks::ui_settings::UiSettings;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::SettingsApp;

mod opening_ui_settings_creates_the_block_once;

fn editor() -> (EditorTest<'static, SettingsApp>, BlockHandle<Settings>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Settings::new());
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::new_v4());
    let mut app = SettingsApp::default();
    app.connect(host, client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block)
}
