use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;

pub trait PresenceKind: Serialize + DeserializeOwned {
    const ID: Uuid;
}

pub const USER_ACTIVE: Uuid = Uuid::from_u128(0x7573_6572_5f61_6374_6976_655f_5f5f_5f5f);

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct UserActive {
    pub color: PresenceColor,
}

impl PresenceKind for UserActive {
    const ID: Uuid = USER_ACTIVE;
}

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
