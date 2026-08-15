use uuid::Uuid;

use super::{Video, VideoClip, VideoOperation};
use crate::block_ref::BlockRef;
use block::Block;

#[test]
fn video_references_each_block_once() {
    let mut video = Video::new();
    let block_id = Uuid::new_v4();
    let other = Uuid::new_v4();
    for (index, block) in [block_id, other, block_id].into_iter().enumerate() {
        Video::apply_operation(
            &mut video,
            &VideoOperation::InsertClip {
                clip: VideoClip::new(BlockRef::Direct(block), 5),
                index,
            },
        );
    }
    assert_eq!(video.clips().len(), 3);
    assert_eq!(video.references(), vec![block_id, other]);
}
