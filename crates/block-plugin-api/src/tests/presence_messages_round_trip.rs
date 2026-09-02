use super::*;

#[test]
fn presence_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::Presence {
            instance: EditorInstanceId(6),
            visible: true,
            entries: vec![PresenceEntry {
                client_id: 7,
                presence_id: [3; 16],
                data: vec![1, 2, 3],
            }],
        }),
        Message::Editor(EditorMessage::PublishPresence {
            instance: EditorInstanceId(6),
            presence_id: [3; 16],
            data: Some(vec![4, 5]),
        }),
        Message::Editor(EditorMessage::PublishPresence {
            instance: EditorInstanceId(6),
            presence_id: [3; 16],
            data: None,
        }),
        Message::Editor(EditorMessage::RevealPresence {
            instance: EditorInstanceId(6),
            client_id: 42,
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }

    let oversized = Message::Editor(EditorMessage::Presence {
        instance: EditorInstanceId(6),
        visible: true,
        entries: vec![
            PresenceEntry {
                client_id: 7,
                presence_id: [3; 16],
                data: Vec::new(),
            };
            MAX_COLLECTION_ITEMS + 1
        ],
    });
    assert_eq!(
        encode_frame(&oversized),
        Err(DecodeError::LimitExceeded("collection"))
    );
}
