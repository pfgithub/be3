use uuid::Uuid;

use super::{Presentation, PresentationOperation, PresentationSlide};
use block::Block;

#[test]
fn presentation_clamps_insert_indices() {
    let slide = PresentationSlide {
        id: Uuid::new_v4(),
        block_id: Uuid::new_v4(),
    };
    let mut presentation = Presentation::new();
    Presentation::apply_operation(
        &mut presentation,
        &PresentationOperation::Insert {
            slide: slide.clone(),
            index: usize::MAX,
        },
    );
    assert_eq!(presentation.slides(), &[slide]);
}
