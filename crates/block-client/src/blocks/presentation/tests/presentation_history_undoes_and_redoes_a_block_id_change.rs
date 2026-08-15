use uuid::Uuid;

use super::{Presentation, PresentationOperation, PresentationSlide};
use crate::block_ref::BlockRef;
use crate::BlockClient;

#[test]
fn presentation_history_undoes_and_redoes_a_block_id_change() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(Presentation::new());
    let slide = PresentationSlide {
        id: Uuid::new_v4(),
        block_id: BlockRef::Direct(Uuid::new_v4()),
    };
    block.operate(PresentationOperation::Insert {
        slide: slide.clone(),
        index: 0,
    });

    let new_block_id = BlockRef::Direct(Uuid::new_v4());
    block.operate(PresentationOperation::SetBlockId {
        slide_id: slide.id,
        block_id: new_block_id,
    });
    assert_eq!(block.read().unwrap().slides()[0].block_id, new_block_id);

    block.undo();
    assert_eq!(block.read().unwrap().slides()[0].block_id, slide.block_id);

    block.redo();
    assert_eq!(block.read().unwrap().slides()[0].block_id, new_block_id);
}
