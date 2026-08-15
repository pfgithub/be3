use block::Block;

use super::*;

#[test]
fn settings_resolve_prefers_client_specific_entry_over_fallback() {
    let block_type = Uuid::new_v4();
    let client_id = Uuid::new_v4();
    let fallback_block = Uuid::new_v4();
    let client_block = Uuid::new_v4();

    let mut settings = Settings::new();
    Settings::apply_operation(
        &mut settings,
        &SettingsOperation::SetEntry {
            block_type,
            activation: ActivationCondition::Fallback,
            block: BlockRef::Direct(fallback_block),
        },
    );
    Settings::apply_operation(
        &mut settings,
        &SettingsOperation::SetEntry {
            block_type,
            activation: ActivationCondition::Client(client_id),
            block: BlockRef::Direct(client_block),
        },
    );

    assert_eq!(
        settings.resolve(block_type, client_id),
        Some(BlockRef::Direct(client_block))
    );
}
