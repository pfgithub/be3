use super::*;

#[test]
fn a_hole_in_the_middle_leaves_a_ring() {
    let whole = rect(0.0, 0.0, 30.0, 30.0);
    let hole = rect(10.0, 10.0, 20.0, 20.0);
    let pieces = subtract(whole, &[hole]);
    assert!(disjoint(&pieces));
    assert_eq!(area(&pieces), 30.0 * 30.0 - 10.0 * 10.0);
    for piece in &pieces {
        assert_eq!(*piece, piece.intersect(whole));
        assert!(!piece.intersect(hole).is_positive());
    }
}
