use super::{sample, Video, VideoAttachment, VideoOperation};
use block::Block;

#[test]
fn video_attachment_cycles_are_refused() {
    let (mut video, first, second, attached) = sample();

    // Hanging a clip off something it already carries would lose it.
    let mut cycled = video.clip(first).unwrap().clone();
    cycled.attachment = Some(VideoAttachment::new(attached, 0));
    Video::apply_operation(
        &mut video,
        &VideoOperation::UpdateClips {
            clips: vec![cycled],
        },
    );
    assert_eq!(video.clip(first).unwrap().attachment, None);

    // So would hanging it off itself.
    let mut looped = video.clip(attached).unwrap().clone();
    looped.attachment = Some(VideoAttachment::new(attached, 4));
    Video::apply_operation(
        &mut video,
        &VideoOperation::UpdateClips {
            clips: vec![looped],
        },
    );
    assert_eq!(
        video.clip(attached).unwrap().attachment,
        Some(VideoAttachment::new(first, 2))
    );

    // Reattaching to another clip is fine, and detaching is always allowed.
    let mut reattached = video.clip(attached).unwrap().clone();
    reattached.attachment = Some(VideoAttachment::new(second, 1));
    Video::apply_operation(
        &mut video,
        &VideoOperation::UpdateClips {
            clips: vec![reattached],
        },
    );
    assert_eq!(video.timing(attached).unwrap().start, 11);

    let mut detached = video.clip(attached).unwrap().clone();
    detached.attachment = None;
    Video::apply_operation(
        &mut video,
        &VideoOperation::UpdateClips {
            clips: vec![detached],
        },
    );
    assert_eq!(video.children(None).len(), 3);
}
