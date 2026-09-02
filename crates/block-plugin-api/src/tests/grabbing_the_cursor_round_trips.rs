use super::*;

#[test]
fn grabbing_the_cursor_round_trips() {
    for message in [
        Message::Editor(EditorMessage::GrabCursor {
            instance: EditorInstanceId(4),
            grabbed: true,
        }),
        Message::Input(InputBatch {
            screen: ScreenId(1),
            events: vec![InputEvent::PointerMotion { x: -3.5, y: 7.25 }],
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
