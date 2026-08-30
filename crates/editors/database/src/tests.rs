use std::sync::Arc;

use block_client::block_ref::BlockRef;
use block_client::blocks::database::Database;
use block_client::blocks::database_schema::DatabaseSchema;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use uuid::Uuid;

use crate::app::DatabaseApp;

mod a_database_without_views_says_so;
mod a_new_database_starts_with_a_name_field;

fn editor() -> (
    EditorTest<'static, DatabaseApp>,
    Arc<BlockClient>,
    BlockHandle<Database>,
) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let schema = client.create_block(DatabaseSchema::new());
    let block = client.create_block(Database::new(BlockRef::Direct(schema.id())));
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = DatabaseApp::default();
    app.connect(host, Arc::clone(&client), block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, client, block)
}
