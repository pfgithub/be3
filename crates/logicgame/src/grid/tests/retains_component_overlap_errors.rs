use super::*;

#[test]
fn retains_component_overlap_errors() {
    let mut grid = LogicGrid::new();
    let first = grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
    let second = grid.add_component(Point::new(0, 0), Rotation::Down, ComponentKind::Led);
    assert!(grid
        .validate()
        .contains(&ValidationError::ComponentOverlap { first, second }));
}
