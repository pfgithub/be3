use uuid::Uuid;

use super::{sample, starts, Video, VideoClip, VideoOperation};
use crate::block_ref::BlockRef;
use block::Block;

#[test]
fn video_attached_clips_start_at_their_offset() {
    let (mut video, first, _, attached) = sample();
    assert_eq!(
        starts(&video).iter().find(|(id, _, _)| *id == attached),
        Some(&(attached, 2, 1))
    );

    // An attachment of an attachment offsets from the clip it hangs off.
    let nested = VideoClip::new(BlockRef::Direct(Uuid::new_v4()), 4).attached_to(attached, 3);
    let nested_id = nested.id;
    Video::apply_operation(
        &mut video,
        &VideoOperation::InsertClip {
            clip: nested,
            index: 0,
        },
    );
    assert_eq!(
        starts(&video).iter().find(|(id, _, _)| *id == nested_id),
        Some(&(nested_id, 5, 2))
    );

    // Moving the clip it all hangs off carries the whole subtree along.
    let mut moved = video.clip(first).unwrap().clone();
    moved.length = 20;
    Video::apply_operation(
        &mut video,
        &VideoOperation::UpdateClips { clips: vec![moved] },
    );
    assert_eq!(video.timing(nested_id).unwrap().start, 5);
    assert_eq!(video.duration(), 25);
}
