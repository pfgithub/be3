use super::*;

#[test]
fn component_bounds_snap_to_largest_port_scale() {
    let mut grid = LogicGrid::new();
    grid.add_component(
        Point::new(2, 3),
        Rotation::Up,
        ComponentKind::MergerSplitter {
            input_scale: scale(8),
            output_scale: scale(8),
        },
    );

    let bounds = grid.bounds().unwrap();

    assert_eq!(bounds.min, Point::new(0, 0));
    assert_eq!(bounds.max, Point::new(16, 16));
}
