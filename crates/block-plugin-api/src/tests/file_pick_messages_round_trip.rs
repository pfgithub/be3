use super::*;

#[test]
fn file_pick_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::PickFile {
            instance: EditorInstanceId(7),
            request_id: 3,
            filter: FileFilter {
                name: "Images".into(),
                default_file_name: "Image".into(),
                extensions: vec!["png".into(), "jpg".into()],
                mime_types: vec!["image/*".into()],
            },
        }),
        Message::Editor(EditorMessage::FilePicked {
            instance: EditorInstanceId(7),
            request_id: 3,
            pick: FilePick::Chosen {
                name: "photo.png".into(),
                data: vec![7; MAX_STRING_BYTES + 1],
            },
        }),
        Message::Editor(EditorMessage::FilePicked {
            instance: EditorInstanceId(7),
            request_id: 4,
            pick: FilePick::Cancelled,
        }),
        Message::Editor(EditorMessage::FilePicked {
            instance: EditorInstanceId(7),
            request_id: 5,
            pick: FilePick::Failed("Could not read photo.png".into()),
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
