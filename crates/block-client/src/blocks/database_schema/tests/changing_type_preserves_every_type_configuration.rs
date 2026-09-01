use super::*;

#[test]
fn changing_type_preserves_every_type_configuration() {
    let mut schema = DatabaseSchema::new();
    let original = field(DatabaseFieldType::Enum);
    let field_id = original.id;
    DatabaseSchema::apply_operation(
        &mut schema,
        &DatabaseSchemaOperation::AddField {
            field: original.clone(),
        },
    );
    DatabaseSchema::apply_operation(
        &mut schema,
        &DatabaseSchemaOperation::SetFieldType {
            field_id,
            field_type: DatabaseFieldType::Block,
        },
    );
    let changed = &schema.fields()[0];
    assert_eq!(changed.enum_options, original.enum_options);
    assert_eq!(changed.number_options, original.number_options);
    assert_eq!(changed.block_options, original.block_options);
}
