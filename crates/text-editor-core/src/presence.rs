use block_client::presence::PresenceKind;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Position;

const TEXT_CURSOR: Uuid = Uuid::from_u128(0x7465_7874_5f63_7572_736f_725f_5f5f_5f5f);

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct TextCursor {
    pub anchor: Position,
    pub focus: Position,
}

impl PresenceKind for TextCursor {
    const ID: Uuid = TEXT_CURSOR;
}
