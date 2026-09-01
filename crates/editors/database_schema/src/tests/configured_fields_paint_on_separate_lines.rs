use super::*;
use block_client::blocks::database_schema::{
    DatabaseBlockOptions, DatabaseEnumOption, DatabaseField, DatabaseFieldType,
    DatabaseNumberOptions, DatabaseNumberScale, DatabaseSchemaOperation,
};

#[test]
fn configured_fields_paint_on_separate_lines() {
    let (mut editor, block) = editor();
    let fields = [
        DatabaseField {
            id: Uuid::new_v4(),
            name: "Status".into(),
            field_type: DatabaseFieldType::Enum,
            enum_options: vec![
                DatabaseEnumOption {
                    id: Uuid::new_v4(),
                    name: "Ready".into(),
                },
                DatabaseEnumOption {
                    id: Uuid::new_v4(),
                    name: "Blocked".into(),
                },
            ],
            number_options: Default::default(),
            block_options: Default::default(),
        },
        DatabaseField {
            id: Uuid::new_v4(),
            name: "Estimate".into(),
            field_type: DatabaseFieldType::Number,
            enum_options: Vec::new(),
            number_options: DatabaseNumberOptions {
                minimum: Some(1.0),
                maximum: Some(100.0),
                step: Some(1.1),
                scale: DatabaseNumberScale::Logarithmic,
            },
            block_options: Default::default(),
        },
        DatabaseField {
            id: Uuid::new_v4(),
            name: "Attachment".into(),
            field_type: DatabaseFieldType::Block,
            enum_options: Vec::new(),
            number_options: Default::default(),
            block_options: DatabaseBlockOptions {
                block_type: Some(Uuid::from_u128(7)),
            },
        },
    ];
    for field in fields {
        block.operate(DatabaseSchemaOperation::AddField { field });
    }
    editor.run();
    editor.snapshot("configured_fields_paint_on_separate_lines");
}
