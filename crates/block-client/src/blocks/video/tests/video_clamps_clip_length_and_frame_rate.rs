use uuid::Uuid;

use super::{Video, VideoClip, VideoFrameRate, VideoOperation, MAX_CLIP_LENGTH};
use crate::block_ref::BlockRef;
use block::Block;

#[test]
fn video_clamps_clip_length_and_frame_rate() {
    let mut video = Video::new();
    assert_eq!(video.frame_rate(), VideoFrameRate::DEFAULT);

    let empty = VideoClip::new(BlockRef::Direct(Uuid::new_v4()), 0);
    let id = empty.id;
    Video::apply_operation(
        &mut video,
        &VideoOperation::InsertClip {
            clip: empty,
            index: 0,
        },
    );
    assert_eq!(video.clip(id).unwrap().length, 1);

    let mut endless = video.clip(id).unwrap().clone();
    endless.length = u64::MAX;
    Video::apply_operation(
        &mut video,
        &VideoOperation::UpdateClips {
            clips: vec![endless],
        },
    );
    assert_eq!(video.clip(id).unwrap().length, MAX_CLIP_LENGTH);

    Video::apply_operation(
        &mut video,
        &VideoOperation::SetFrameRate {
            frame_rate: VideoFrameRate::new(0, 0),
        },
    );
    assert_eq!(video.frame_rate(), VideoFrameRate::new(1, 1));

    Video::apply_operation(
        &mut video,
        &VideoOperation::SetFrameRate {
            frame_rate: VideoFrameRate::new(30_000, 1001),
        },
    );
    assert!((video.frame_rate().frames_per_second() - 29.97).abs() < 0.001);
    assert_eq!(video.frame_rate().frames(2.0), 59);
}
