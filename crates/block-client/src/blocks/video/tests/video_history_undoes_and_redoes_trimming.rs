use uuid::Uuid;

use super::{Video, VideoClip, VideoOperation};
use crate::block_ref::BlockRef;
use crate::BlockClient;

#[test]
fn video_history_undoes_and_redoes_trimming() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(Video::new());
    let clip = VideoClip::new(BlockRef::Direct(Uuid::new_v4()), 10);
    let id = clip.id;
    block.operate(VideoOperation::InsertClip { clip, index: 0 });

    let mut trimmed = block.read().unwrap().clip(id).unwrap().clone();
    trimmed.length = 4;
    block.operate(VideoOperation::UpdateClips {
        clips: vec![trimmed],
    });
    assert_eq!(block.read().unwrap().duration(), 4);

    block.undo();
    assert_eq!(block.read().unwrap().duration(), 10);
    block.redo();
    assert_eq!(block.read().unwrap().duration(), 4);

    // Undoing past the trim removes the clip the insert added.
    block.undo();
    block.undo();
    assert!(block.read().unwrap().clips().is_empty());
}
