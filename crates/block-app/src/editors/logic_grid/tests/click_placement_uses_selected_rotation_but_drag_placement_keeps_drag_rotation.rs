use super::*;

#[test]
fn click_placement_uses_selected_rotation_but_drag_placement_keeps_drag_rotation() {
    assert_eq!(
        placement_rotation(
            [4.5, 4.5],
            [4.5, 4.5],
            ComponentOrientation::Left,
            ToolKind::Not
        ),
        ComponentOrientation::Left
    );
    assert_eq!(
        placement_rotation(
            [4.5, 4.5],
            [8.0, 4.5],
            ComponentOrientation::Left,
            ToolKind::Not
        ),
        ComponentOrientation::Right
    );
    assert_eq!(
        placement_rotation(
            [4.5, 4.5],
            [8.0, 4.5],
            ComponentOrientation::Left,
            ToolKind::Input
        ),
        ComponentOrientation::Left
    );
}
