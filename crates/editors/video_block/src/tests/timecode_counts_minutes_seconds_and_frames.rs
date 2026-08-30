use super::*;

#[test]
fn timecode_counts_minutes_seconds_and_frames() {
    let rate = VideoFrameRate::new(24, 1);
    assert_eq!(timecode(rate, 0), "0:00.00");
    assert_eq!(timecode(rate, 25), "0:01.01");
}
