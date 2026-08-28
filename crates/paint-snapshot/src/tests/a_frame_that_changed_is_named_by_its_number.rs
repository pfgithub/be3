use super::*;

#[test]
fn a_frame_that_changed_is_named_by_its_number() {
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    let before = triangles(&[red, red, red]);
    let after = triangles(&[red, blue, red]);

    let difference = crate::difference(&before, &after).unwrap();
    assert_eq!(difference.frame, Some(1));
    assert!(difference.description.starts_with("frame 2 of 3 changed:"));

    let shorter = crate::difference(&before, &triangles(&[red])).unwrap();
    assert_eq!(shorter.frame, None);
    assert_eq!(
        shorter.description,
        "the recording is one frame long, it used to be 3 frames"
    );

    assert!(crate::difference(&before, &triangles(&[red, red, red])).is_none());
}
