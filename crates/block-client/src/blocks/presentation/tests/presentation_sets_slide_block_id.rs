use uuid::Uuid;

use super::{Presentation, PresentationOperation, PresentationSlide};
use block::Block;

#[test]
fn presentation_sets_slide_block_id() {
    let slide = PresentationSlide {
        id: Uuid::new_v4(),
        block_id: Uuid::new_v4(),
    };
    let mut presentation = Presentation::new();
    Presentation::apply_operation(
        &mut presentation,
        &PresentationOperation::Insert {
            slide: slide.clone(),
            index: 0,
        },
    );

    let new_block_id = Uuid::new_v4();
    Presentation::apply_operation(
        &mut presentation,
        &PresentationOperation::SetBlockId {
            slide_id: slide.id,
            block_id: new_block_id,
        },
    );
    assert_eq!(
        presentation.slides(),
        &[PresentationSlide {
            id: slide.id,
            block_id: new_block_id,
        }]
    );

    Presentation::apply_operation(
        &mut presentation,
        &PresentationOperation::SetBlockId {
            slide_id: Uuid::new_v4(),
            block_id: Uuid::new_v4(),
        },
    );
    assert_eq!(presentation.slides().len(), 1);
}
