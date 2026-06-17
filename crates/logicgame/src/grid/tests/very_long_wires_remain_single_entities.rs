use super::*;

#[test]
fn very_long_wires_remain_single_entities() {
    let mut grid = LogicGrid::new();
    grid.add_wire(wire((-10_000_000_000_000, 0), (10_000_000_000_000, 0), 1));
    assert_eq!(grid.wires().len(), 1);
    assert_eq!(grid.wires()[0].length(), 20_000_000_000_000);
}
