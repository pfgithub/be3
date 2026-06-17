use super::*;

#[test]
fn empty_grid_has_no_bounds() {
    assert_eq!(LogicGrid::new().bounds(), None);
}
