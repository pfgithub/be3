use super::*;

#[test]
fn rejects_malformed_payload() {
    let frame = [0, 0, 0, 1, 255];
    assert_eq!(decode_frame(&frame), Err(DecodeError::MalformedPayload));
}
