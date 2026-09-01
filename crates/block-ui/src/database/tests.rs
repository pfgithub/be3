use super::*;
use block_client::blocks::database_schema::DatabaseEnumOption;

fn field(field_type: DatabaseFieldType) -> DatabaseField {
    DatabaseField {
        id: Uuid::new_v4(),
        name: "Field".to_owned(),
        field_type,
        enum_options: Vec::new(),
        number_options: Default::default(),
        block_options: Default::default(),
    }
}
mod enum_value_formats_as_option_name;
mod number_parsing_accepts_valid_and_rejects_invalid_and_empty;
mod string_empty_is_stored;
mod value_state_distinguishes_uniform_absent_and_mixed_values;

use block_client::{
    block_ref::BlockRef,
    blocks::{
        database::{DatabaseColor, DatabaseValue},
        database_schema::{DatabaseNumberOptions, DatabaseNumberScale},
    },
};
mod color_datetime_and_boolean_text_round_trip;
mod invalid_typed_values_do_not_produce_replacements;
mod typed_numbers_clamp_to_configured_boundaries;
mod unresolved_block_references_use_stable_fallback_ids;
