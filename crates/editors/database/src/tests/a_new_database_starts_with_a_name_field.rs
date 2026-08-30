use super::*;

#[test]
fn a_new_database_starts_with_a_name_field() {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let mut app = DatabaseApp::default();
    app.connect_creation(EditorHost::default(), Arc::clone(&client));

    let id = app.create_block().unwrap();

    let database = client.get_block::<Database>(id);
    let schema_id = database.read().unwrap().schema_id().as_direct().unwrap();
    let schema = client.get_block::<DatabaseSchema>(schema_id);
    let schema = schema.read().unwrap();
    let fields = schema.fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "Name");
}
