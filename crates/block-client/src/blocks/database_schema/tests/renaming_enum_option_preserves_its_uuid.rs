use super::*;

#[test]
fn renaming_enum_option_preserves_its_uuid() {
    let field_id = Uuid::new_v4();
    let option_id = Uuid::new_v4();
    let mut schema = DatabaseSchema::new();
    DatabaseSchema::apply_operation(
        &mut schema,
        &DatabaseSchemaOperation::AddField {
            field: DatabaseField {
                id: field_id,
                name: "Status".into(),
                field_type: DatabaseFieldType::Enum,
                options: Vec::new(),
            },
        },
    );
    DatabaseSchema::apply_operation(
        &mut schema,
        &DatabaseSchemaOperation::AddEnumOption {
            field_id,
            option: DatabaseEnumOption {
                id: option_id,
                name: "Old name".into(),
            },
        },
    );
    DatabaseSchema::apply_operation(
        &mut schema,
        &DatabaseSchemaOperation::RenameEnumOption {
            field_id,
            option_id,
            name: "New name".into(),
        },
    );

    assert_eq!(schema.fields()[0].options[0].id, option_id);
    assert_eq!(schema.fields()[0].options[0].name, "New name");
}
