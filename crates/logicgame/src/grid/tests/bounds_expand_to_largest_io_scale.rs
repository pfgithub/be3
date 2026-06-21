use super::*;

#[test]
fn bounds_expand_to_largest_io_scale() {
    let mut grid = LogicGrid::new();
    grid.add_component(
        Point::new(2, 3),
        Rotation::Up,
        ComponentKind::MergerSplitter {
            input_scale: scale(8),
            output_scale: scale(8),
        },
    );
    grid.add_component(
        Point::new(0, -8),
        Rotation::Up,
        ComponentKind::Input {
            scale: scale(4),
            id: InputId::from_u128(1),
            label: String::new(),
        },
    );

    let bounds = grid.bounds().unwrap();

    assert_eq!(bounds.min, Point::new(0, 0));
    assert_eq!(bounds.max, Point::new(12, 12));
}
