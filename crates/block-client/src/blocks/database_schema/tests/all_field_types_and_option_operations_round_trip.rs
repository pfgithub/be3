use super::*;

#[test]
fn all_field_types_and_option_operations_round_trip() {
    let field_types = [
        DatabaseFieldType::String,
        DatabaseFieldType::Number,
        DatabaseFieldType::Enum,
        DatabaseFieldType::Block,
        DatabaseFieldType::Boolean,
        DatabaseFieldType::Color,
        DatabaseFieldType::Datetime,
    ];
    assert_eq!(
        serde_json::from_str::<Vec<DatabaseFieldType>>(
            &serde_json::to_string(&field_types).unwrap()
        )
        .unwrap(),
        field_types
    );

    let field_id = Uuid::new_v4();
    let operations = [
        DatabaseSchemaOperation::SetNumberOptions {
            field_id,
            options: DatabaseNumberOptions {
                minimum: Some(1.0),
                maximum: Some(100.0),
                step: Some(1.1),
                scale: DatabaseNumberScale::Logarithmic,
            },
        },
        DatabaseSchemaOperation::SetBlockOptions {
            field_id,
            options: DatabaseBlockOptions {
                block_type: Some(Uuid::new_v4()),
            },
        },
    ];
    for operation in operations {
        assert_eq!(
            serde_json::from_str::<DatabaseSchemaOperation>(
                &serde_json::to_string(&operation).unwrap()
            )
            .unwrap(),
            operation
        );
    }
}
