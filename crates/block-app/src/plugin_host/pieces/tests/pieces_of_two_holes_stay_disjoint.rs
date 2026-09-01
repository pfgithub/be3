use super::*;

#[test]
fn pieces_of_two_holes_stay_disjoint() {
    let whole = rect(0.0, 0.0, 40.0, 40.0);
    let holes = [rect(5.0, 5.0, 15.0, 35.0), rect(10.0, 20.0, 30.0, 25.0)];
    let pieces = subtract(whole, &holes);
    assert!(disjoint(&pieces));
    let covered = 10.0 * 30.0 + 20.0 * 5.0 - 5.0 * 5.0;
    assert_eq!(area(&pieces), 40.0 * 40.0 - covered);
    for piece in &pieces {
        for hole in &holes {
            assert!(!piece.intersect(*hole).is_positive());
        }
    }
}
