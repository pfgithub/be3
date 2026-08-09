use block::Block;
use uuid::Uuid;

use super::{
    DatabaseEnumOption, DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
};

mod enum_options_are_added_and_removed_by_uuid;
mod renaming_enum_option_preserves_its_uuid;
mod renaming_field_preserves_its_uuid;
mod schema_adds_and_removes_fields_by_uuid;
