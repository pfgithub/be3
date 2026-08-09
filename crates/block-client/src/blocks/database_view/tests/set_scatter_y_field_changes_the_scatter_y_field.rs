use super::*;

#[test]
fn set_scatter_y_field_changes_the_scatter_y_field() {
    let field_id = Uuid::new_v4();
    let mut view = DatabaseView::new(Uuid::new_v4());
    assert_eq!(view.scatter_y_field_id(), None);

    DatabaseView::apply_operation(
        &mut view,
        &DatabaseViewOperation::SetScatterYField {
            field_id: Some(field_id),
        },
    );
    assert_eq!(view.scatter_y_field_id(), Some(field_id));

    DatabaseView::apply_operation(
        &mut view,
        &DatabaseViewOperation::SetScatterYField { field_id: None },
    );
    assert_eq!(view.scatter_y_field_id(), None);
}
