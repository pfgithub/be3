use super::*;

#[test]
fn resize_messages_round_trip() {
    let message = Message::Editor(EditorMessage::Resized {
        instance: EditorInstanceId(11),
        width: 320.5,
        height: 240.25,
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
