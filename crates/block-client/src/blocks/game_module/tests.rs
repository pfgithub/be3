use super::*;

mod game_module_implicit_name_uses_source_name;
mod game_module_replace_operation_replaces_the_module;
mod game_module_serialization_round_trips_the_bytes;

fn wasm_bytes() -> Vec<u8> {
    vec![0, 97, 115, 109, 1, 0, 0, 0]
}
