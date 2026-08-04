use super::*;

#[test]
fn gate_drag_maps_to_rotation_and_placement_anchor() {
    let anchor = Point::new(8, 8);
    assert_eq!(
        drag_rotation([9.5, 9.5], [13.0, 9.0]),
        Some(Rotation::Right)
    );
    assert_eq!(drag_rotation([9.5, 9.5], [8.5, 9.5]), Some(Rotation::Left));
    assert_eq!(drag_rotation([9.5, 9.5], [9.5, 8.5]), Some(Rotation::Up));
    assert_eq!(drag_rotation([9.5, 9.5], [9.5, 10.5]), Some(Rotation::Down));
    assert_eq!(
        drag_rotation([9.5, 9.5], [9.500_001, 9.5]),
        Some(Rotation::Right)
    );
    assert_eq!(drag_rotation([9.5, 9.5], [9.5, 9.5]), None);
    assert_eq!(
        component_placement_position(anchor, Rotation::Right, scale(2), ToolKind::Not),
        Point::new(8, 8)
    );
    assert_eq!(
        component_placement_position(anchor, Rotation::Down, scale(2), ToolKind::Not),
        Point::new(8, 8)
    );
    assert_eq!(
        component_placement_position(anchor, Rotation::Up, scale(2), ToolKind::Not),
        Point::new(8, 6)
    );
    assert_eq!(
        component_placement_position(anchor, Rotation::Left, scale(2), ToolKind::Not),
        Point::new(6, 8)
    );
}
