use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::presence::PresenceKind;

use super::CanvasPoint;

/// The well-known presence id for [`CanvasCursor`]. Never reuse this value
/// for another presence kind.
const CANVAS_CURSOR: Uuid = Uuid::from_u128(0x6361_6e76_6173_5f63_7572_736f_725f_5f5f);

/// Posted by a client while it has this canvas open on screen, so other
/// clients editing at the same time can see its pointer and selection.
/// Posted every frame the block is on screen and cleared as soon as it
/// isn't, mirroring the lifecycle of
/// [`UserActive`](crate::presence::UserActive) presence for the same block.
/// Carries no color of its own: painting code matches each entry's
/// `ClientId` against that same client's `UserActive` entry to find its
/// color.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasCursor {
    pub pointer: Option<CanvasPoint>,
    pub selection: Vec<Uuid>,
}

impl PresenceKind for CanvasCursor {
    const ID: Uuid = CANVAS_CURSOR;
}
