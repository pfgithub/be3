use super::*;

mod video_attached_clips_start_at_their_offset;
mod video_attachment_cycles_are_refused;
mod video_base_clips_run_back_to_back;
mod video_clamps_clip_length_and_frame_rate;
mod video_history_undoes_and_redoes_a_rippling_removal;
mod video_history_undoes_and_redoes_trimming;
mod video_references_each_block_once;
mod video_removing_a_base_clip_ripples_and_takes_its_attachments;
mod video_serialization_round_trips;
mod video_visible_clips_are_ordered_from_the_base_up;

/// A video with two base clips and one clip attached to the first, which is
/// the smallest arrangement that shows both rippling and attachment offsets.
fn sample() -> (Video, Uuid, Uuid, Uuid) {
    let mut video = Video::new();
    let first = VideoClip::new(Uuid::new_v4(), 10);
    let second = VideoClip::new(Uuid::new_v4(), 5);
    let attached = VideoClip::new(Uuid::new_v4(), 3).attached_to(first.id, 2);
    let ids = (first.id, second.id, attached.id);
    for (index, clip) in [first, second, attached].into_iter().enumerate() {
        Video::apply_operation(&mut video, &VideoOperation::InsertClip { clip, index });
    }
    (video, ids.0, ids.1, ids.2)
}

fn starts(video: &Video) -> Vec<(Uuid, u64, usize)> {
    video
        .timeline()
        .iter()
        .map(|timing| (timing.id, timing.start, timing.depth))
        .collect()
}
