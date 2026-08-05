use uuid::Uuid;

use super::{Video, VideoClip, VideoOperation};
use crate::BlockClient;

#[test]
fn video_history_undoes_and_redoes_a_rippling_removal() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(Video::new());
    let first = VideoClip::new(Uuid::new_v4(), 10);
    let second = VideoClip::new(Uuid::new_v4(), 5);
    let attached = VideoClip::new(Uuid::new_v4(), 3).attached_to(first.id, 2);
    let (first_id, attached_id) = (first.id, attached.id);
    for (index, clip) in [first, second, attached].into_iter().enumerate() {
        block.operate(VideoOperation::InsertClip { clip, index });
    }
    let before = block.read().unwrap().timeline();

    block.operate(VideoOperation::RemoveClips {
        ids: vec![first_id],
    });
    assert_eq!(block.read().unwrap().clips().len(), 1);

    // Undoing puts the clip and everything hanging off it back where it was.
    block.undo();
    assert_eq!(block.read().unwrap().timeline(), before);
    assert_eq!(
        block.read().unwrap().clip(attached_id).unwrap().attachment,
        Some(super::VideoAttachment::new(first_id, 2))
    );

    block.redo();
    let after = block.read().unwrap();
    assert_eq!(after.clips().len(), 1);
    assert!(after.clip(attached_id).is_none());
}
