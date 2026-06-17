use super::*;

#[test]
fn square_components_stay_on_the_clicked_cell_for_every_rotation() {
    let anchor = Point::new(8, 8);
    for kind in [ToolKind::MergerSplitter, ToolKind::Input, ToolKind::Output] {
        for rotation in [
            Rotation::Up,
            Rotation::Right,
            Rotation::Down,
            Rotation::Left,
        ] {
            assert_eq!(
                component_placement_position(anchor, rotation, scale(2), kind),
                anchor
            );
        }
    }
}
