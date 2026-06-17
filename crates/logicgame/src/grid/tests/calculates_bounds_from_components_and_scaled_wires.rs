use super::*;

#[test]
fn calculates_bounds_from_components_and_scaled_wires() {
    let mut grid = LogicGrid::new();
    grid.add_component(
        Point::new(-6, -4),
        Rotation::Right,
        ComponentKind::Not { scale: scale(2) },
    );
    grid.add_wire(wire((3, 5), (11, 5), 4));

    let bounds = grid.bounds().unwrap();
    assert_eq!(bounds.min, Point::new(-6, -4));
    assert_eq!(bounds.max, Point::new(15, 9));
    assert_eq!(bounds.width(), 21);
    assert_eq!(bounds.height(), 13);
    assert_eq!(bounds.area(), 273);
}
