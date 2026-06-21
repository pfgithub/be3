use super::*;

#[test]
fn component_orientation_canonicalizes_flips() {
    assert_eq!(
        ComponentOrientation::Up.flip_horizontal(),
        ComponentOrientation::UpMirrored
    );
    assert_eq!(
        ComponentOrientation::UpMirrored.flip_horizontal(),
        ComponentOrientation::Up
    );
    assert_eq!(
        ComponentOrientation::Up.flip_vertical(),
        ComponentOrientation::DownMirrored
    );
    assert_eq!(
        ComponentOrientation::Right.flip_vertical(),
        ComponentOrientation::LeftMirrored
    );
    assert_eq!(
        ComponentOrientation::Up.flip_horizontal().flip_vertical(),
        ComponentOrientation::Down
    );
}
