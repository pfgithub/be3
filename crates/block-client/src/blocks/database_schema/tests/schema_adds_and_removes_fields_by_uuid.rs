use super::*;

#[test]
fn schema_adds_and_removes_fields_by_uuid() {
    let retained = DatabaseField {
        id: Uuid::new_v4(),
        name: "Retained".into(),
        field_type: DatabaseFieldType::String,
        options: Vec::new(),
    };
    let removed = DatabaseField {
        id: Uuid::new_v4(),
        name: "Removed".into(),
        field_type: DatabaseFieldType::Number,
        options: Vec::new(),
    };
    let mut schema = DatabaseSchema::new();
    for field in [retained.clone(), removed.clone()] {
        DatabaseSchema::apply_operation(&mut schema, &DatabaseSchemaOperation::AddField { field });
    }
    DatabaseSchema::apply_operation(
        &mut schema,
        &DatabaseSchemaOperation::RemoveField {
            field_id: removed.id,
        },
    );

    assert_eq!(schema.fields(), &[retained]);
}
