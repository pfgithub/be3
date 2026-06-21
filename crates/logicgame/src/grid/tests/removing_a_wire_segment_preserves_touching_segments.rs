use super::*;

#[test]
fn removing_a_wire_segment_preserves_touching_segments() {
    let mut grid = LogicGrid::new();
    grid.add_wire(wire((1, 1), (3, 1), 1));
    grid.add_wire(wire((3, 1), (3, 3), 1));
    grid.add_wire(wire((3, 1), (5, 1), 1));

    grid.remove_wire_segment(wire((3, 1), (5, 1), 1));

    assert_eq!(
        grid.wires(),
        &[wire((1, 1), (3, 1), 1), wire((3, 1), (3, 3), 1)]
    );
}
