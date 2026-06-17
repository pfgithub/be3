use super::*;

#[test]
fn does_not_merge_collinear_wires_whose_endpoints_only_touch() {
    let mut grid = LogicGrid::new();
    grid.add_wire(wire((1, 1), (1, 2), 1));
    grid.add_wire(wire((1, 3), (1, 4), 1));

    assert_eq!(
        grid.wires(),
        &[wire((1, 1), (1, 2), 1), wire((1, 3), (1, 4), 1)]
    );
}
