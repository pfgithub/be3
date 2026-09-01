use super::*;

#[test]
fn drag_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::DragOver {
            instance: EditorInstanceId(4),
            region: EditorRegion::Frame,
            x: 12.5,
            y: 30.0,
            block_id: [1; 16],
            block_type: [2; 16],
            dropped: true,
        }),
        Message::Editor(EditorMessage::DragLeft {
            instance: EditorInstanceId(4),
        }),
        Message::Editor(EditorMessage::DragAccepted {
            instance: EditorInstanceId(4),
            accepted: true,
        }),
        Message::Editor(EditorMessage::IntrinsicSize {
            instance: EditorInstanceId(4),
            width: 400.0,
            height: 264.0,
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
