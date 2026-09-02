use super::*;

#[test]
fn component_preview_shortcuts_rotate_and_scale_selected_placement() {
    assert_eq!(
        ComponentOrientation::Up.rotate_left(),
        ComponentOrientation::Left
    );
    assert_eq!(
        ComponentOrientation::UpMirrored.rotate_left(),
        ComponentOrientation::RightMirrored
    );
    assert_eq!(
        ComponentOrientation::Up.rotate_right(),
        ComponentOrientation::Right
    );

    assert_eq!(previous_scale(scale(1)), scale(1));
    assert_eq!(previous_scale(scale(8)), scale(4));
    assert_eq!(next_scale(scale(8)), scale(16));
    assert_eq!(next_scale(scale(64)), scale(64));
}
