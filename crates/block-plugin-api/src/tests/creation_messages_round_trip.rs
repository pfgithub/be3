use super::*;

#[test]
fn creation_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::OpenCreation {
            instance: EditorInstanceId(2),
        }),
        Message::Editor(EditorMessage::CreationContent {
            instance: EditorInstanceId(2),
            payload: Some("x".repeat(MAX_STRING_BYTES + 1)),
        }),
        Message::Editor(EditorMessage::CreationContent {
            instance: EditorInstanceId(2),
            payload: None,
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
