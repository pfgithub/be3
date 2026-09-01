use super::*;

#[test]
fn number_options_are_normalized_when_added_and_updated() {
    let mut schema = DatabaseSchema::new();
    let mut number = field(DatabaseFieldType::Number);
    let field_id = number.id;
    number.number_options = DatabaseNumberOptions {
        minimum: Some(f64::INFINITY),
        maximum: Some(f64::NAN),
        step: Some(0.0),
        scale: DatabaseNumberScale::Linear,
    };
    DatabaseSchema::apply_operation(
        &mut schema,
        &DatabaseSchemaOperation::AddField { field: number },
    );
    assert_eq!(schema.fields()[0].number_options, DatabaseNumberOptions::default());

    DatabaseSchema::apply_operation(
        &mut schema,
        &DatabaseSchemaOperation::SetNumberOptions {
            field_id,
            options: DatabaseNumberOptions {
                minimum: Some(20.0),
                maximum: Some(10.0),
                step: Some(-1.0),
                scale: DatabaseNumberScale::Linear,
            },
        },
    );
    assert_eq!(
        schema.fields()[0].number_options,
        DatabaseNumberOptions {
            minimum: Some(10.0),
            maximum: Some(20.0),
            step: None,
            scale: DatabaseNumberScale::Linear,
        }
    );

    DatabaseSchema::apply_operation(
        &mut schema,
        &DatabaseSchemaOperation::SetNumberOptions {
            field_id,
            options: DatabaseNumberOptions {
                minimum: Some(-1.0),
                maximum: Some(0.0),
                step: Some(1.0),
                scale: DatabaseNumberScale::Logarithmic,
            },
        },
    );
    assert_eq!(
        schema.fields()[0].number_options,
        DatabaseNumberOptions {
            minimum: None,
            maximum: None,
            step: None,
            scale: DatabaseNumberScale::Logarithmic,
        }
    );
}
