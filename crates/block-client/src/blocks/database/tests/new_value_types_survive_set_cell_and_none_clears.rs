use super::*;

#[test]
fn new_value_types_survive_set_cell_and_none_clears() {
    let schema_id = Uuid::new_v4();
    let mut database = Database::new(BlockRef::Direct(schema_id));
    let values = [
        DatabaseValue::Boolean(false),
        DatabaseValue::Color(DatabaseColor {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 0,
        }),
        DatabaseValue::Datetime(0),
        DatabaseValue::Datetime(-1),
        DatabaseValue::Block(BlockRef::Direct(Uuid::new_v4())),
        DatabaseValue::Block(BlockRef::RepoRelative {
            repo: Uuid::new_v4(),
            eternal_id: Uuid::new_v4(),
        }),
    ];

    for (row_index, value) in values.into_iter().enumerate() {
        let field_id = Uuid::new_v4();
        Database::apply_operation(
            &mut database,
            &DatabaseOperation::SetCell {
                row_index,
                field_id,
                value: Some(value.clone()),
            },
        );
        assert_eq!(database.rows()[row_index].value(field_id), Some(&value));
        Database::apply_operation(
            &mut database,
            &DatabaseOperation::SetCell {
                row_index,
                field_id,
                value: None,
            },
        );
        assert_eq!(database.rows().get(row_index).and_then(|row| row.value(field_id)), None);
    }
}
