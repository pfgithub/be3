use block::Block;
use uuid::Uuid;

use super::{
    DatabaseBlockOptions, DatabaseEnumOption, DatabaseField, DatabaseFieldType,
    DatabaseNumberOptions, DatabaseNumberScale, DatabaseSchema, DatabaseSchemaOperation,
};

mod enum_options_are_added_and_removed_by_uuid;
mod renaming_enum_option_preserves_its_uuid;
mod renaming_field_preserves_its_uuid;
mod schema_adds_and_removes_fields_by_uuid;


fn field(field_type: DatabaseFieldType) -> DatabaseField {
    DatabaseField {
        id: Uuid::new_v4(),
        name: "Field".into(),
        field_type,
        enum_options: vec![DatabaseEnumOption {
            id: Uuid::new_v4(),
            name: "Option".into(),
        }],
        number_options: DatabaseNumberOptions {
            minimum: Some(1.0),
            maximum: Some(10.0),
            step: Some(2.0),
            scale: DatabaseNumberScale::Linear,
        },
        block_options: DatabaseBlockOptions {
            block_type: Some(Uuid::new_v4()),
        },
    }
}
mod all_field_types_and_option_operations_round_trip;
mod number_options_are_normalized_when_added_and_updated;
mod changing_type_preserves_every_type_configuration;
