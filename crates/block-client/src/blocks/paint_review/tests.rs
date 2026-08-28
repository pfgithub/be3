use block::Block;
use uuid::Uuid;

use crate::block_ref::BlockRef;

use super::{ApprovedPainting, PaintReview, PaintReviewOperation};

mod approving_a_path_again_replaces_what_was_approved;
mod deleting_a_snapshot_forgets_the_path_it_held;
mod forgetting_a_path_leaves_the_rest;
mod serialization_round_trip;
