use super::*;

#[test]
fn enum_options_are_added_and_removed_by_uuid() {
    let field_id = Uuid::new_v4();
    let retained = DatabaseEnumOption {
        id: Uuid::new_v4(),
        name: "Retained".into(),
    };
    let removed = DatabaseEnumOption {
        id: Uuid::new_v4(),
        name: "Removed".into(),
    };
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
    for option in [retained.clone(), removed.clone()] {
        DatabaseSchema::apply_operation(
            &mut schema,
            &DatabaseSchemaOperation::AddEnumOption { field_id, option },
        );
    }
    DatabaseSchema::apply_operation(
        &mut schema,
        &DatabaseSchemaOperation::RemoveEnumOption {
            field_id,
            option_id: removed.id,
        },
    );

    assert_eq!(schema.fields()[0].options, vec![retained]);
}
