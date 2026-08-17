use super::*;

#[test]
fn multiplexed_messages_round_trip() {
    let editor = Message::Editor(EditorMessage::Close {
        instance: EditorInstanceId(7),
    });
    let client = Message::Client(DelegatedClientMessage::Watch {
        instance: EditorInstanceId(7),
        request_id: 9,
        block_id: [2; 16],
        block_type: [3; 16],
    });
    assert_eq!(
        decode_frame(&encode_frame(&editor).unwrap()).unwrap(),
        editor
    );
    assert_eq!(
        decode_frame(&encode_frame(&client).unwrap()).unwrap(),
        client
    );
}
