use super::*;

#[test]
fn rejects_unknown_message_kind() {
    let payload = u32::MAX.to_le_bytes();
    let mut frame = (payload.len() as u32).to_be_bytes().to_vec();
    frame.extend_from_slice(&payload);
    assert_eq!(decode_frame(&frame), Err(DecodeError::MalformedPayload));
}
