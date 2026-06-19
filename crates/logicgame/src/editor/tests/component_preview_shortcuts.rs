use super::*;

#[test]
fn component_preview_shortcuts_rotate_and_scale_selected_placement() {
    assert_eq!(rotate_left(Rotation::Up), Rotation::Left);
    assert_eq!(rotate_right(Rotation::Up), Rotation::Right);

    assert_eq!(previous_scale(scale(1)), scale(1));
    assert_eq!(previous_scale(scale(8)), scale(4));
    assert_eq!(next_scale(scale(8)), scale(16));
    assert_eq!(next_scale(scale(64)), scale(64));
}

#[test]
fn click_placement_uses_selected_rotation_but_drag_placement_keeps_drag_rotation() {
    assert_eq!(
        placement_rotation([4.5, 4.5], [4.5, 4.5], Rotation::Left, ToolKind::Not),
        Rotation::Left
    );
    assert_eq!(
        placement_rotation([4.5, 4.5], [8.0, 4.5], Rotation::Left, ToolKind::Not),
        Rotation::Right
    );
    assert_eq!(
        placement_rotation([4.5, 4.5], [8.0, 4.5], Rotation::Left, ToolKind::Input),
        Rotation::Left
    );
}
