use super::*;

#[test]
fn wire_deletion_uses_pointer_position_at_intersections() {
    let left = wire((1, 1), (3, 1), 1);
    let right = wire((3, 1), (5, 1), 1);
    let down = wire((3, 1), (3, 3), 1);
    let wires = [left, right, down];

    assert_eq!(deletion_wire(&wires, [3.8, 1.5], 0.1), Some(right));
    assert_eq!(deletion_wire(&wires, [3.2, 1.5], 0.1), Some(left));
    assert_eq!(deletion_wire(&wires, [3.5, 1.8], 0.1), Some(down));
}
