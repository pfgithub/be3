use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

/// A well-known kind of presence value, identified by a fixed [`ID`](Self::ID)
/// so unrelated presence data posted for the same block never collides.
pub trait PresenceKind: Serialize + DeserializeOwned {
    const ID: Uuid;
}

/// The well-known presence id for [`UserActive`]. Never reuse this value for
/// another presence kind.
pub const USER_ACTIVE: Uuid = Uuid::from_u128(0x7573_6572_5f61_6374_6976_655f_5f5f_5f5f);

/// Posted by a client while one of its tabs has the block open on screen, so
/// other clients can show who else is currently looking at it. Cleared as
/// soon as the tab is no longer on screen.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct UserActive {
    pub color: PresenceColor,
}

impl PresenceKind for UserActive {
    const ID: Uuid = USER_ACTIVE;
}

/// A small fixed palette used to tell simultaneous editors of a block apart
/// in the UI. Colors are assigned per active client, not per account, so the
/// same person editing from two tabs may be shown in two different colors.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum PresenceColor {
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
}

impl PresenceColor {
    pub const ALL: [PresenceColor; 8] = [
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Teal,
        Self::Blue,
        Self::Purple,
        Self::Pink,
    ];
}

/// Picks a color not already in `used`, or a random one from the palette if
/// every color is already taken.
pub fn pick_free_color(used: impl IntoIterator<Item = PresenceColor>) -> PresenceColor {
    let used: std::collections::HashSet<_> = used.into_iter().collect();
    PresenceColor::ALL
        .into_iter()
        .find(|color| !used.contains(color))
        .unwrap_or_else(|| {
            let index = (Uuid::new_v4().as_u128() % PresenceColor::ALL.len() as u128) as usize;
            PresenceColor::ALL[index]
        })
}

#[cfg(test)]
mod tests;
