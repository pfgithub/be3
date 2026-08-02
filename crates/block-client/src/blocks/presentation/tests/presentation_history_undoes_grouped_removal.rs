use uuid::Uuid;

use super::{Presentation, PresentationOperation, PresentationSlide};
use crate::BlockClient;

#[test]
fn presentation_history_undoes_grouped_removal() {
    let client = BlockClient::new(Uuid::new_v4());
    let block = client.create_block(Presentation::new());
    let repeated = Uuid::new_v4();
    let slides = [repeated, Uuid::new_v4(), repeated]
        .into_iter()
        .map(|block_id| PresentationSlide {
            id: Uuid::new_v4(),
            block_id,
        })
        .collect::<Vec<_>>();
    for (index, slide) in slides.iter().cloned().enumerate() {
        block.operate(PresentationOperation::Insert { slide, index });
    }
    block.operate_grouped(
        slides
            .iter()
            .filter(|slide| slide.block_id == repeated)
            .map(|slide| PresentationOperation::Remove { slide_id: slide.id }),
    );
    assert_eq!(block.read().unwrap().slides(), &slides[1..2]);
    block.undo();
    assert_eq!(block.read().unwrap().slides(), slides);
    block.redo();
    assert_eq!(block.read().unwrap().slides(), &slides[1..2]);
}
