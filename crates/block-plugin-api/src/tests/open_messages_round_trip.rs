use super::*;

#[test]
fn open_messages_round_trip() {
    let message = Message::Editor(EditorMessage::Open {
        instance: EditorInstanceId(9),
        block_id: [1; 16],
        block_type: [2; 16],
        account_id: [3; 16],
        workspace_id: [4; 16],
        client_id: [5; 16],
        editable: true,
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
