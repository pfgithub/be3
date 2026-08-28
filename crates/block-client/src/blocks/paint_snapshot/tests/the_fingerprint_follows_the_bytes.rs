use super::*;

#[test]
fn the_fingerprint_follows_the_bytes() {
    let snapshot = PaintSnapshot::new("a.paint", vec![7; 32]);
    assert_eq!(snapshot.hash(), PaintSnapshot::fingerprint(snapshot.data()));
    assert_eq!(snapshot.hash().len(), 64);
    assert_ne!(snapshot.hash(), PaintSnapshot::fingerprint(&[7; 33]));
    assert_eq!(
        PaintSnapshot::new("b.paint", vec![7; 32]).hash(),
        snapshot.hash()
    );
}
