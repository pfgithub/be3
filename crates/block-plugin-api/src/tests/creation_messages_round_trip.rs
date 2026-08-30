use super::*;

#[test]
fn creation_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::OpenCreation {
            instance: EditorInstanceId(2),
            account_id: [3; 16],
            workspace_id: [4; 16],
            client_id: [5; 16],
        }),
        Message::Editor(EditorMessage::CreationReady {
            instance: EditorInstanceId(2),
            ready: true,
        }),
        Message::Editor(EditorMessage::CommitCreation {
            instance: EditorInstanceId(2),
        }),
        Message::Editor(EditorMessage::CreationBlock {
            instance: EditorInstanceId(2),
            outcome: CreationOutcome::Created([5; 16]),
        }),
        Message::Editor(EditorMessage::CreationBlock {
            instance: EditorInstanceId(2),
            outcome: CreationOutcome::Failed("no file was chosen".into()),
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
