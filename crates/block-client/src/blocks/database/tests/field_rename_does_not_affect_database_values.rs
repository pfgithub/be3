use super::*;

#[test]
fn field_rename_does_not_affect_database_values() {
    let field_id = Uuid::new_v4();
    let mut database = Database::new(BlockRef::Direct(Uuid::new_v4()));
    Database::apply_operation(
        &mut database,
        &DatabaseOperation::SetCell {
            row_index: 0,
            field_id,
            value: Some(DatabaseValue::String("Ada".into())),
        },
    );

    assert_eq!(
        database.rows()[0].value(field_id),
        Some(&DatabaseValue::String("Ada".into()))
    );
}
