use uuid::Uuid;

use super::{Presentation, PresentationOperation, PresentationSlide};
use crate::block_ref::BlockRef;
use crate::BlockClient;

#[test]
fn presentation_history_undoes_and_redoes_changes() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(Presentation::new());
    let first = PresentationSlide {
        id: Uuid::new_v4(),
        block_id: BlockRef::Direct(Uuid::new_v4()),
    };
    let second = PresentationSlide {
        id: Uuid::new_v4(),
        block_id: BlockRef::Direct(Uuid::new_v4()),
    };
    block.operate(PresentationOperation::Insert {
        slide: first.clone(),
        index: 0,
    });
    block.operate(PresentationOperation::Insert {
        slide: second.clone(),
        index: 1,
    });
    block.operate(PresentationOperation::Move {
        slide_id: second.id,
        index: 0,
    });
    assert_eq!(
        block.read().unwrap().slides(),
        &[second.clone(), first.clone()]
    );
    block.undo();
    assert_eq!(
        block.read().unwrap().slides(),
        &[first.clone(), second.clone()]
    );
    block.undo();
    assert_eq!(block.read().unwrap().slides(), std::slice::from_ref(&first));
    block.redo();
    block.redo();
    assert_eq!(block.read().unwrap().slides(), &[second, first]);
}
