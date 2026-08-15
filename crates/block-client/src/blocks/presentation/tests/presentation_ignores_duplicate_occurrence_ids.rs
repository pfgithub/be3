use uuid::Uuid;

use super::{Presentation, PresentationOperation, PresentationSlide};
use crate::block_ref::BlockRef;
use block::Block;

#[test]
fn presentation_ignores_duplicate_occurrence_ids() {
    let id = Uuid::new_v4();
    let first = PresentationSlide {
        id,
        block_id: BlockRef::Direct(Uuid::new_v4()),
    };
    let duplicate = PresentationSlide {
        id,
        block_id: BlockRef::Direct(Uuid::new_v4()),
    };
    let mut presentation = Presentation::new();
    Presentation::apply_operation(
        &mut presentation,
        &PresentationOperation::Insert {
            slide: first.clone(),
            index: 0,
        },
    );
    Presentation::apply_operation(
        &mut presentation,
        &PresentationOperation::Insert {
            slide: duplicate,
            index: 0,
        },
    );
    assert_eq!(presentation.slides(), &[first]);
}
