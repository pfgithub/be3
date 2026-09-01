use std::sync::Arc;

use block_client::{
    block_ref::BlockRef,
    blocks::{
        database::{Database, DatabaseColor, DatabaseOperation, DatabaseValue},
        database_schema::{
            DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
        },
        database_view::DatabaseView,
    },
    BlockClient,
};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;

use crate::app::DatabaseViewApp;
use uuid::Uuid;

fn typed_editor() -> (
    EditorTest<'static, DatabaseViewApp>,
    block_client::BlockHandle<Database>,
    Uuid,
) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let schema = client.create_block(DatabaseSchema::new());
    let boolean_id = Uuid::new_v4();
    let fields = [
        (boolean_id, "Done", DatabaseFieldType::Boolean),
        (Uuid::new_v4(), "Tint", DatabaseFieldType::Color),
        (Uuid::new_v4(), "When", DatabaseFieldType::Datetime),
        (Uuid::new_v4(), "Related", DatabaseFieldType::Block),
    ];
    for (id, name, field_type) in fields {
        schema.operate(DatabaseSchemaOperation::AddField {
            field: DatabaseField {
                id,
                name: name.into(),
                field_type,
                enum_options: Vec::new(),
                number_options: Default::default(),
                block_options: Default::default(),
            },
        });
    }
    let database = client.create_block(Database::new(BlockRef::Direct(schema.id())));
    let field_ids = schema
        .read()
        .unwrap()
        .fields()
        .iter()
        .map(|field| field.id)
        .collect::<Vec<_>>();
    for (field_id, value) in [
        (field_ids[0], DatabaseValue::Boolean(false)),
        (
            field_ids[1],
            DatabaseValue::Color(DatabaseColor {
                red: 0x10,
                green: 0x20,
                blue: 0x30,
                alpha: 0x40,
            }),
        ),
        (field_ids[2], DatabaseValue::Datetime(1_709_251_500)),
        (
            field_ids[3],
            DatabaseValue::Block(BlockRef::Direct(Uuid::from_u128(42))),
        ),
    ] {
        database.operate(DatabaseOperation::SetCell {
            row_index: 0,
            field_id,
            value: Some(value),
        });
    }
    let view = client.create_block(DatabaseView::new(BlockRef::Direct(database.id())));
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = DatabaseViewApp::default();
    app.connect(host, client, view.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, database, boolean_id)
}

#[test]
fn new_type_cells_render_and_boolean_toggles() {
    let (mut editor, database, boolean_id) = typed_editor();
    editor
        .find(&format!("database-view.cell.0.{boolean_id}"))
        .click();
    editor.run();
    assert_eq!(
        database.read().unwrap().rows()[0].value(boolean_id),
        Some(&DatabaseValue::Boolean(true))
    );
    editor.snapshot("new_type_cells_render_and_boolean_toggles");
}
