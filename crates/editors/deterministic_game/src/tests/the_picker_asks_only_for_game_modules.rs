use super::*;

#[test]
fn the_picker_asks_only_for_game_modules() {
    let filter = module_filter();

    assert_eq!(filter.block_types, [GameModule::TYPE_ID.into_bytes()]);
    assert!(!filter.templates);
}
