use super::*;

#[test]
fn ime_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::Ime {
            instance: EditorInstanceId(6),
            region: EditorRegion::Frame,
            area: Some(ImeArea {
                rect: ChildRect {
                    x: 1.0,
                    y: 2.0,
                    width: 3.0,
                    height: 4.0,
                },
                cursor: ChildRect {
                    x: 5.0,
                    y: 6.0,
                    width: 1.0,
                    height: 12.0,
                },
            }),
        }),
        Message::Editor(EditorMessage::Ime {
            instance: EditorInstanceId(6),
            region: EditorRegion::Frame,
            area: None,
        }),
        Message::Input(InputBatch {
            screen: ScreenId(2),
            events: vec![
                InputEvent::Ime(ImeInput::Enabled),
                InputEvent::Ime(ImeInput::Preedit("か".into())),
                InputEvent::Ime(ImeInput::Commit("漢".into())),
                InputEvent::Ime(ImeInput::Disabled),
            ],
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
