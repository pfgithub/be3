use super::*;

#[test]
fn set_sort_replaces_or_clears_the_sort() {
    let field_id = Uuid::new_v4();
    let mut view = DatabaseView::new(Uuid::new_v4());

    let sort = DatabaseViewSort {
        field_id,
        direction: SortDirection::Ascending,
    };
    DatabaseView::apply_operation(
        &mut view,
        &DatabaseViewOperation::SetSort { sort: Some(sort) },
    );
    assert_eq!(view.sort(), Some(sort));

    let sort = DatabaseViewSort {
        field_id,
        direction: SortDirection::Descending,
    };
    DatabaseView::apply_operation(
        &mut view,
        &DatabaseViewOperation::SetSort { sort: Some(sort) },
    );
    assert_eq!(view.sort(), Some(sort));

    DatabaseView::apply_operation(&mut view, &DatabaseViewOperation::SetSort { sort: None });
    assert_eq!(view.sort(), None);
}
