use super::*;

#[test]
fn rejects_truncated_frame() {
    let frame = [0, 0, 0, 8, 0];
    assert_eq!(
        decode_frame(&frame),
        Err(DecodeError::TruncatedFrame {
            expected: 12,
            available: 5
        })
    );
}
