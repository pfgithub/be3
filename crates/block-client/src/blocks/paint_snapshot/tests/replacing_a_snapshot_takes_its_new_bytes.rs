use super::*;

#[test]
fn replacing_a_snapshot_takes_its_new_bytes() {
    let mut snapshot = PaintSnapshot::new("a.paint", vec![1, 2, 3]);
    let replacement = PaintSnapshot::new("a.paint", vec![4, 5, 6, 7]);
    PaintSnapshot::apply_operation(
        &mut snapshot,
        &PaintSnapshotOperation::Replace {
            snapshot: replacement.clone(),
        },
    );
    assert_eq!(snapshot, replacement);
    assert_eq!(snapshot.data(), [4, 5, 6, 7]);
    assert_eq!(snapshot.implicit_name().as_deref(), Some("a.paint"));
}
