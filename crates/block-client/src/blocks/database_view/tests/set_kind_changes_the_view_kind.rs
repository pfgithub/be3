use super::*;

#[test]
fn set_kind_changes_the_view_kind() {
    let mut view = DatabaseView::new(BlockRef::Direct(Uuid::new_v4()));
    assert_eq!(view.kind(), DatabaseViewKind::Spreadsheet);

    DatabaseView::apply_operation(
        &mut view,
        &DatabaseViewOperation::SetKind {
            kind: DatabaseViewKind::Kanban,
        },
    );
    assert_eq!(view.kind(), DatabaseViewKind::Kanban);
}
