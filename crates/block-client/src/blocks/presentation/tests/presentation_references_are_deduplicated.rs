use uuid::Uuid;

use super::{Presentation, PresentationOperation, PresentationSlide};
use block::Block;

#[test]
fn presentation_references_are_deduplicated() {
    let repeated = Uuid::new_v4();
    let other = Uuid::new_v4();
    let mut presentation = Presentation::new();
    for (index, block_id) in [repeated, other, repeated].into_iter().enumerate() {
        Presentation::apply_operation(
            &mut presentation,
            &PresentationOperation::Insert {
                slide: PresentationSlide {
                    id: Uuid::new_v4(),
                    block_id,
                },
                index,
            },
        );
    }
    assert_eq!(presentation.references(), vec![repeated, other]);
}
