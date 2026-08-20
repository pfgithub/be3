use super::*;

#[test]
fn rejects_oversized_block_payload() {
    let message = Message::Client(TunnelMessage::Response {
        instance: EditorInstanceId(1),
        payload: "x".repeat(MAX_BLOCK_PAYLOAD_BYTES + 1),
    });
    assert_eq!(
        encode_frame(&message),
        Err(DecodeError::LimitExceeded("block payload"))
    );
}
