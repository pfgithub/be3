use super::*;

#[test]
fn nothing_is_left_when_a_hole_covers_everything() {
    let whole = rect(0.0, 0.0, 10.0, 10.0);
    assert!(subtract(whole, &[rect(-5.0, -5.0, 15.0, 15.0)]).is_empty());
    assert_eq!(subtract(whole, &[]), vec![whole]);
    assert!(subtract(rect(0.0, 0.0, 0.0, 10.0), &[]).is_empty());
    assert_eq!(
        subtract(whole, &[rect(20.0, 20.0, 30.0, 30.0)]),
        vec![whole]
    );
}
