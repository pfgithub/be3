use block::Block;

use super::*;

#[test]
fn settings_resolve_falls_back_when_no_client_entry_exists() {
    let block_type = Uuid::new_v4();
    let fallback_block = Uuid::new_v4();

    let mut settings = Settings::new();
    Settings::apply_operation(
        &mut settings,
        &SettingsOperation::SetEntry {
            block_type,
            activation: ActivationCondition::Fallback,
            block: BlockRef::Direct(fallback_block),
        },
    );

    assert_eq!(
        settings.resolve(block_type, Uuid::new_v4()),
        Some(BlockRef::Direct(fallback_block))
    );
}
