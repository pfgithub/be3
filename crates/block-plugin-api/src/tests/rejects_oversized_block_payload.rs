use super::*;

#[test]
fn rejects_oversized_block_payload() {
    let message = Message::Client(DelegatedClientMessage::Operate {
        instance: EditorInstanceId(1),
        request_id: 2,
        block_id: [0; 16],
        operation_id: [0; 16],
        sequence: 1,
        operation: vec![0; MAX_BLOCK_PAYLOAD_BYTES + 1],
    });
    assert_eq!(
        encode_frame(&message),
        Err(DecodeError::LimitExceeded("block payload"))
    );
}
