use std::collections::BTreeMap;

use block::Block;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Which client a [`SettingEntry`] applies to. `Client` lets different
/// devices keep different settings for the same block type; `Fallback` is
/// used when nothing more specific has been registered.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationCondition {
    Fallback,
    Client(Uuid),
}

/// One settings block registered against a block type, active under
/// `activation`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SettingEntry {
    pub activation: ActivationCondition,
    pub block: Uuid,
}

/// The workspace's single root settings block. Rather than each block type
/// getting its own block at the root, every type registers the block that
/// holds its settings here, keyed by its own [`Block::TYPE_ID`].
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct Settings {
    entries: BTreeMap<Uuid, Vec<SettingEntry>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum SettingsOperation {
    /// Registers `block` as the settings for `block_type` under
    /// `activation`, replacing whatever was previously registered for that
    /// same pair.
    SetEntry {
        block_type: Uuid,
        activation: ActivationCondition,
        block: Uuid,
    },
}

impl Settings {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every entry registered for `block_type`, across all activation
    /// conditions.
    pub fn entries(&self, block_type: Uuid) -> &[SettingEntry] {
        self.entries.get(&block_type).map_or(&[], Vec::as_slice)
    }

    /// The block registered for `block_type` that applies to `client_id`,
    /// preferring an entry registered for that exact client over the
    /// fallback one.
    pub fn resolve(&self, block_type: Uuid, client_id: Uuid) -> Option<Uuid> {
        let entries = self.entries(block_type);
        entries
            .iter()
            .find(|entry| entry.activation == ActivationCondition::Client(client_id))
            .or_else(|| {
                entries
                    .iter()
                    .find(|entry| entry.activation == ActivationCondition::Fallback)
            })
            .map(|entry| entry.block)
    }
}

impl Block for Settings {
    type Operation = SettingsOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7365_7474_696e_6773_2d62_6c6f_636b_3031);

    fn apply_operation(settings: &mut Self, operation: &Self::Operation) {
        match operation {
            SettingsOperation::SetEntry {
                block_type,
                activation,
                block,
            } => {
                let entries = settings.entries.entry(*block_type).or_default();
                entries.retain(|entry| entry.activation != *activation);
                entries.push(SettingEntry {
                    activation: *activation,
                    block: *block,
                });
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.entries
            .values()
            .flatten()
            .map(|entry| entry.block)
            .collect()
    }
}

#[cfg(test)]
mod tests;
