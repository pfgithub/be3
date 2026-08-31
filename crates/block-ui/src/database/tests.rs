use super::*;
use block_client::blocks::database_schema::DatabaseEnumOption;

fn field(field_type: DatabaseFieldType) -> DatabaseField {
    DatabaseField {
        id: Uuid::new_v4(),
        name: "Field".to_owned(),
        field_type,
        options: Vec::new(),
    }
}
mod enum_value_formats_as_option_name;
mod number_parsing_accepts_valid_and_rejects_invalid_and_empty;
mod string_empty_is_stored;
mod value_state_distinguishes_uniform_absent_and_mixed_values;
