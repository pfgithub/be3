use super::*;

#[test]
fn view_messages_round_trip() {
    let messages = [
        Message::Editor(EditorMessage::ViewChanged {
            instance: EditorInstanceId(3),
            x: -12.5,
            y: 4.0,
            width: 320.0,
            height: 240.5,
            scale: 1.5,
        }),
        Message::Editor(EditorMessage::ChangeView {
            instance: EditorInstanceId(3),
            change: ViewChange::Pan { x: 8.0, y: -3.5 },
        }),
        Message::Editor(EditorMessage::ChangeView {
            instance: EditorInstanceId(3),
            change: ViewChange::Zoom {
                factor: 1.25,
                anchor: Some((10.0, 20.0)),
            },
        }),
        Message::Editor(EditorMessage::ChangeView {
            instance: EditorInstanceId(3),
            change: ViewChange::Zoom {
                factor: 0.8,
                anchor: None,
            },
        }),
        Message::Editor(EditorMessage::ChangeView {
            instance: EditorInstanceId(3),
            change: ViewChange::Fit,
        }),
    ];
    for message in messages {
        let frame = encode_frame(&message).expect("the message encodes");
        assert_eq!(decode_frame(&frame).expect("the frame decodes"), message);
    }
}
