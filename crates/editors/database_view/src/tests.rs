use std::sync::Arc;

use block_client::block_ref::BlockRef;
use block_client::blocks::database::{Database, DatabaseValue};
use block_client::blocks::database_schema::{
    DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
};
use block_client::blocks::database_view::{DatabaseView, DatabaseViewKind};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::DatabaseViewApp;

mod a_new_view_starts_as_a_spreadsheet;
mod selected_row_string_edit_updates_the_database;
mod switching_to_kanban_stores_the_kind;

fn editor() -> (
    EditorTest<'static, DatabaseViewApp>,
    BlockHandle<DatabaseView>,
    BlockHandle<Database>,
    Uuid,
) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let schema = client.create_block(DatabaseSchema::new());
    let field_id = Uuid::new_v4();
    schema.operate(DatabaseSchemaOperation::AddField {
        field: DatabaseField {
            id: field_id,
            name: "Name".into(),
            field_type: DatabaseFieldType::String,
            options: Vec::new(),
        },
    });
    let database = client.create_block(Database::new(BlockRef::Direct(schema.id())));
    let block = client.create_block(DatabaseView::new(BlockRef::Direct(database.id())));
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = DatabaseViewApp::default();
    app.connect(host, client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block, database, field_id)
}
