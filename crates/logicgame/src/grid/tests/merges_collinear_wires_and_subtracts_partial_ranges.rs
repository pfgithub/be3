use super::*;

#[test]
fn merges_collinear_wires_and_subtracts_partial_ranges() {
    let mut grid = LogicGrid::new();
    grid.add_wire(wire((0, 0), (8, 0), 1));
    grid.add_wire(wire((6, 0), (20, 0), 1));
    assert_eq!(grid.wires(), &[wire((0, 0), (20, 0), 1)]);

    grid.remove_wire(wire((5, 0), (15, 0), 1));
    assert_eq!(
        grid.wires(),
        &[wire((0, 0), (4, 0), 1), wire((16, 0), (20, 0), 1)]
    );
}
