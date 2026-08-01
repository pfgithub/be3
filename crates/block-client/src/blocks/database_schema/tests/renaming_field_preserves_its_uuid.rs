use super::*;

#[test]
fn renaming_field_preserves_its_uuid() {
    let id = Uuid::new_v4();
    let mut schema = DatabaseSchema::new();
    DatabaseSchema::apply_operation(
        &mut schema,
        &DatabaseSchemaOperation::AddField {
            field: DatabaseField {
                id,
                name: "Old name".into(),
                field_type: DatabaseFieldType::String,
            },
        },
    );
    DatabaseSchema::apply_operation(
        &mut schema,
        &DatabaseSchemaOperation::RenameField {
            field_id: id,
            name: "New name".into(),
        },
    );

    assert_eq!(schema.fields()[0].id, id);
    assert_eq!(schema.fields()[0].name, "New name");
}
