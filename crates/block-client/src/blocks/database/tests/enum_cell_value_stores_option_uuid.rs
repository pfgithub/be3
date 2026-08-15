use super::*;

#[test]
fn enum_cell_value_stores_option_uuid() {
    let field_id = Uuid::new_v4();
    let option_id = Uuid::new_v4();
    let mut database = Database::new(BlockRef::Direct(Uuid::new_v4()));
    Database::apply_operation(
        &mut database,
        &DatabaseOperation::SetCell {
            row_index: 0,
            field_id,
            value: Some(DatabaseValue::Enum(option_id)),
        },
    );

    assert_eq!(
        database.rows()[0].value(field_id),
        Some(&DatabaseValue::Enum(option_id))
    );
}
