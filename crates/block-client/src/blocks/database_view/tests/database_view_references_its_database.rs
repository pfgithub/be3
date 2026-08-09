use super::*;

#[test]
fn database_view_references_its_database() {
    let database_id = Uuid::new_v4();
    let view = DatabaseView::new(database_id);

    assert_eq!(view.references(), vec![database_id]);
}
