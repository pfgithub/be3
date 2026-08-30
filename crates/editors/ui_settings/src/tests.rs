use std::sync::Arc;

use block_client::blocks::ui_settings::UiSettings;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{egui, App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::UiSettingsApp;

mod pressing_an_arrow_key_stores_a_new_zoom;

fn editor() -> (EditorTest<'static, UiSettingsApp>, BlockHandle<UiSettings>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(UiSettings::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = UiSettingsApp::default();
    app.connect(host, client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block)
}
