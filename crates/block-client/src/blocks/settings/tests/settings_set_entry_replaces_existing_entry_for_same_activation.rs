use block::Block;

use super::*;

#[test]
fn settings_set_entry_replaces_existing_entry_for_same_activation() {
    let block_type = Uuid::new_v4();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();

    let mut settings = Settings::new();
    Settings::apply_operation(
        &mut settings,
        &SettingsOperation::SetEntry {
            block_type,
            activation: ActivationCondition::Fallback,
            block: BlockRef::Direct(first),
        },
    );
    Settings::apply_operation(
        &mut settings,
        &SettingsOperation::SetEntry {
            block_type,
            activation: ActivationCondition::Fallback,
            block: BlockRef::Direct(second),
        },
    );

    assert_eq!(settings.entries(block_type).len(), 1);
    assert_eq!(
        settings.resolve(block_type, Uuid::new_v4()),
        Some(BlockRef::Direct(second))
    );
}
