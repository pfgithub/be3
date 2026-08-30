use super::*;

#[test]
fn clipboard_messages_round_trip() {
    let messages = [
        Message::Editor(EditorMessage::PasteImage {
            instance: EditorInstanceId(12),
            request_id: 3,
        }),
        Message::Editor(EditorMessage::ImagePasted {
            instance: EditorInstanceId(12),
            request_id: 3,
            image: ClipboardImage::Pasted {
                name: "Pasted Image.png".into(),
                data: vec![1, 2, 3],
            },
        }),
        Message::Editor(EditorMessage::ImagePasted {
            instance: EditorInstanceId(12),
            request_id: 4,
            image: ClipboardImage::Empty,
        }),
        Message::Input(InputBatch {
            screen: ScreenId(1),
            events: vec![InputEvent::Paste("pasted".into())],
        }),
    ];
    for message in messages {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
