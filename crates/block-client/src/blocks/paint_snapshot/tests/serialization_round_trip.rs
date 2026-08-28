use super::*;

#[test]
fn serialization_round_trip() {
    let snapshot = PaintSnapshot::new("counter.a.paint", vec![1, 2, 3, 0]);
    let encoded = serde_json::to_vec(&snapshot).unwrap();
    assert_eq!(
        serde_json::from_slice::<PaintSnapshot>(&encoded).unwrap(),
        snapshot
    );
}
