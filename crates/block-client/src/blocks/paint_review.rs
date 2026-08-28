use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block_ref::BlockRef;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ApprovedPainting {
    pub path: String,
    pub hash: String,
    pub snapshot: BlockRef,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct PaintReview {
    approved: Vec<ApprovedPainting>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum PaintReviewOperation {
    Approve { painting: ApprovedPainting },
    Forget { path: String },
}

impl PaintReview {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn approved(&self) -> &[ApprovedPainting] {
        &self.approved
    }

    pub fn approval(&self, path: &str) -> Option<&ApprovedPainting> {
        self.approved
            .iter()
            .find(|painting| painting.path == *path)
    }
}

impl Block for PaintReview {
    type Operation = PaintReviewOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7061_696e_742d_7265_7669_6577_2d62_0001);

    fn apply_operation(review: &mut Self, operation: &Self::Operation) {
        match operation {
            PaintReviewOperation::Approve { painting } => {
                match review
                    .approved
                    .iter_mut()
                    .find(|approved| approved.path == painting.path)
                {
                    Some(approved) => *approved = painting.clone(),
                    None => {
                        let index = review
                            .approved
                            .partition_point(|approved| approved.path < painting.path);
                        review.approved.insert(index, painting.clone());
                    }
                }
            }
            PaintReviewOperation::Forget { path } => {
                review.approved.retain(|approved| approved.path != *path);
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.approved
            .iter()
            .filter_map(|approved| approved.snapshot.as_direct())
            .collect()
    }

    fn delete_child(&self, block_id: Uuid) -> Option<Vec<Self::Operation>> {
        Some(
            self.approved
                .iter()
                .filter(|approved| approved.snapshot.as_direct() == Some(block_id))
                .map(|approved| PaintReviewOperation::Forget {
                    path: approved.path.clone(),
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests;
