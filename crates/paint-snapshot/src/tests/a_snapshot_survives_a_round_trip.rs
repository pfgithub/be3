use super::*;

#[test]
fn a_snapshot_survives_a_round_trip() {
    let snapshot = triangle([255, 0, 0, 255]);
    let bytes = snapshot.encode().unwrap();
    assert_eq!(Snapshot::decode(&bytes).unwrap(), snapshot);
    assert!(Snapshot::decode(b"not a snapshot").is_err());
}
