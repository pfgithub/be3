use super::*;

#[test]
fn a_new_view_starts_as_a_spreadsheet() {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let mut app = DatabaseViewApp::default();
    app.connect_creation(EditorHost::default(), Arc::clone(&client));

    let id = app.create_block().unwrap();

    let view = client.get_block::<DatabaseView>(id);
    assert_eq!(view.read().unwrap().kind(), DatabaseViewKind::Spreadsheet);
}
