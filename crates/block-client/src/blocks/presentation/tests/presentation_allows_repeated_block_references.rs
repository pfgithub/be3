use uuid::Uuid;

use super::{Presentation, PresentationOperation, PresentationSlide};
use block::Block;

#[test]
fn presentation_allows_repeated_block_references() {
    let block_id = Uuid::new_v4();
    let mut presentation = Presentation::new();
    for index in 0..2 {
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
    assert_eq!(presentation.slides().len(), 2);
    assert_eq!(
        presentation.slides()[0].block_id,
        presentation.slides()[1].block_id
    );
}
