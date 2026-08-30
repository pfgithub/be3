use std::sync::Arc;

use block_client::blocks::database_schema::DatabaseSchema;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::DatabaseSchemaApp;

mod adding_a_field_appends_a_string_field;

fn editor() -> (
    EditorTest<'static, DatabaseSchemaApp>,
    BlockHandle<DatabaseSchema>,
) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(DatabaseSchema::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = DatabaseSchemaApp::default();
    app.connect(host, client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block)
}
