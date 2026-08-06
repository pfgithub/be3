use block_client::presence::PresenceKind;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Position;

/// The well-known presence id for [`TextCursor`]. Never reuse this value for
/// another presence kind.
const TEXT_CURSOR: Uuid = Uuid::from_u128(0x7465_7874_5f63_7572_736f_725f_5f5f_5f5f);

/// Posted by a client while its cursor or selection is inside this text
/// block, so other clients editing at the same time can see where. Posted
/// every frame the block is on screen and cleared as soon as it isn't,
/// mirroring the lifecycle of
/// [`UserActive`](block_client::presence::UserActive) presence for the same
/// block. Carries no color of its own: painting code matches each entry's
/// `ClientId` against that same client's `UserActive` entry to find its
/// color.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct TextCursor {
    pub anchor: Position,
    pub focus: Position,
}

impl PresenceKind for TextCursor {
    const ID: Uuid = TEXT_CURSOR;
}
