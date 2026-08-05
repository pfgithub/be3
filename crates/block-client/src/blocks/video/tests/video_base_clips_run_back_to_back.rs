use super::{sample, starts};

#[test]
fn video_base_clips_run_back_to_back() {
    let (video, first, second, _) = sample();
    let starts = starts(&video);
    assert_eq!(starts[0], (first, 0, 0));
    assert_eq!(
        starts.iter().find(|(id, _, _)| *id == second),
        Some(&(second, 10, 0))
    );
    assert_eq!(video.duration(), 15);
}
