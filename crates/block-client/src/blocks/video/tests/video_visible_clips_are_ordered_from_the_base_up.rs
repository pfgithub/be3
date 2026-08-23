use super::sample;

#[test]
fn video_visible_clips_are_ordered_from_the_base_up() {
    let (video, first, second, attached) = sample();
                                                                             
    assert_eq!(video.visible_at(2), vec![first, attached]);
    assert_eq!(video.visible_at(0), vec![first]);
    assert_eq!(video.visible_at(5), vec![first]);
    assert_eq!(video.visible_at(10), vec![second]);
    assert!(video.visible_at(15).is_empty());
}
