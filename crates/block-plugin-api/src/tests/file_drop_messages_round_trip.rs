use super::*;

#[test]
fn file_drop_messages_round_trip() {
    let messages = [
        Message::Editor(EditorMessage::FileDrop {
            instance: EditorInstanceId(13),
            region: EditorRegion::Main,
            x: 4.0,
            y: 5.0,
            files: Vec::new(),
            dropped: false,
        }),
        Message::Editor(EditorMessage::FileDrop {
            instance: EditorInstanceId(13),
            region: EditorRegion::Main,
            x: 4.0,
            y: 5.0,
            files: vec![DroppedFile {
                name: "photo.png".into(),
                data: vec![9, 9, 9],
            }],
            dropped: true,
        }),
        Message::Editor(EditorMessage::FileDropLeft {
            instance: EditorInstanceId(13),
        }),
    ];
    for message in messages {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
