use uuid::Uuid;

use super::{Presentation, PresentationOperation, PresentationSlide};
use crate::block_ref::BlockRef;
use block::Block;

#[test]
fn presentation_inserts_removes_and_moves_slides() {
    let first = PresentationSlide {
        id: Uuid::new_v4(),
        block_id: BlockRef::Direct(Uuid::new_v4()),
    };
    let second = PresentationSlide {
        id: Uuid::new_v4(),
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
            slide: second.clone(),
            index: 1,
        },
    );
    Presentation::apply_operation(
        &mut presentation,
        &PresentationOperation::Move {
            slide_id: second.id,
            index: 0,
        },
    );
    assert_eq!(presentation.slides(), &[second.clone(), first.clone()]);
    Presentation::apply_operation(
        &mut presentation,
        &PresentationOperation::Remove {
            slide_id: second.id,
        },
    );
    assert_eq!(presentation.slides(), &[first]);
}
