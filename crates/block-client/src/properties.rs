use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The well-known property key for a block's display name. Never reuse this
/// value for another property.
pub const NAME: Uuid = Uuid::from_u128(0x6e61_6d65_5f5f_5f5f_5f5f_5f5f_5f5f_5f5f);

/// How long a name may be. This is a client-side UI concern - the server
/// only enforces a much larger generic cap on any property value.
pub const MAX_NAME_BYTES: usize = 128;

/// The client's interpretation of the `NAME` property. Once a block has been
/// manually renamed, automatic re-derivation from its content stops touching
/// this property.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BlockName {
    pub manual: bool,
    pub value: String,
}

/// Decodes the `NAME` property, if the block has one.
pub fn read_name(properties: &BTreeMap<Uuid, Vec<u8>>) -> Option<BlockName> {
    let bytes = properties.get(&NAME)?;
    serde_json::from_slice(bytes).ok()
}

pub(crate) fn encode_name(name: &BlockName) -> Vec<u8> {
    serde_json::to_vec(name)
        .unwrap_or_else(|error| crate::fatal(format!("failed to encode block name: {error}")))
}

/// Applies a freshly-derived automatic name to `properties`, unless the
/// block has already been manually renamed - in which case the manual name
/// is left untouched. `implicit_name` is `None` when the block type has
/// nothing useful to auto-name, or when its current content no longer
/// yields one; either way, the `NAME` property is removed rather than left
/// stale.
pub(crate) fn apply_implicit_name(
    properties: &mut BTreeMap<Uuid, Vec<u8>>,
    implicit_name: Option<String>,
) {
    if read_name(properties).is_some_and(|name| name.manual) {
        return;
    }
    match implicit_name {
        Some(value) => {
            properties.insert(
                NAME,
                encode_name(&BlockName {
                    manual: false,
                    value,
                }),
            );
        }
        None => {
            properties.remove(&NAME);
        }
    }
}

#[cfg(test)]
mod tests;
