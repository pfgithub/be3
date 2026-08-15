use super::*;

#[test]
fn set_kanban_field_changes_the_kanban_field() {
    let field_id = Uuid::new_v4();
    let mut view = DatabaseView::new(BlockRef::Direct(Uuid::new_v4()));
    assert_eq!(view.kanban_field_id(), None);

    DatabaseView::apply_operation(
        &mut view,
        &DatabaseViewOperation::SetKanbanField {
            field_id: Some(field_id),
        },
    );
    assert_eq!(view.kanban_field_id(), Some(field_id));

    DatabaseView::apply_operation(
        &mut view,
        &DatabaseViewOperation::SetKanbanField { field_id: None },
    );
    assert_eq!(view.kanban_field_id(), None);
}
