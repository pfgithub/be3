use super::{sample, starts, Video, VideoOperation};
use block::Block;

#[test]
fn video_removing_a_base_clip_ripples_and_takes_its_attachments() {
    let (mut video, first, second, attached) = sample();
    Video::apply_operation(
        &mut video,
        &VideoOperation::RemoveClips { ids: vec![first] },
    );
    // Base clips run back to back, so removing one closes the gap by itself
    // and the clips hanging off it go with it.
    assert_eq!(starts(&video), vec![(second, 0, 0)]);
    assert!(video.clip(attached).is_none());
    assert_eq!(video.duration(), 5);
}
