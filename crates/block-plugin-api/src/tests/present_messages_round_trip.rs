use super::*;

#[test]
fn present_messages_round_trip() {
    for presenting in [true, false] {
        for message in [
            Message::Editor(EditorMessage::Present {
                instance: EditorInstanceId(6),
                presenting,
            }),
            Message::Editor(EditorMessage::PresentingChanged {
                instance: EditorInstanceId(6),
                presenting,
            }),
        ] {
            assert_eq!(
                decode_frame(&encode_frame(&message).unwrap()).unwrap(),
                message
            );
        }
    }
}
