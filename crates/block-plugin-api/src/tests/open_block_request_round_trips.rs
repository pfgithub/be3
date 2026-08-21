use super::*;

#[test]
fn open_block_request_round_trips() {
    let message = Message::Editor(EditorMessage::OpenBlock {
        instance: EditorInstanceId(3),
        block_id: [1; 16],
        block_type: [2; 16],
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
