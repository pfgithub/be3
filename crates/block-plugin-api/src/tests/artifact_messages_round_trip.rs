use super::*;

#[test]
fn artifact_messages_round_trip() {
    let messages = [
        Message::Editor(EditorMessage::OpenArtifact {
            instance: EditorInstanceId(3),
            block_id: [1; 16],
            block_type: [2; 16],
            account_id: [3; 16],
            workspace_id: [4; 16],
            client_id: [5; 16],
            data: b"{\"scale\":2}".to_vec(),
        }),
        Message::Editor(EditorMessage::ArtifactSettings {
            instance: EditorInstanceId(3),
            data: b"{\"scale\":4}".to_vec(),
        }),
        Message::Editor(EditorMessage::ArtifactDescribed {
            instance: EditorInstanceId(3),
            description: ArtifactDescription::Described {
                source: [5; 16],
                summary: "PNG export at 4x".to_owned(),
            },
        }),
        Message::Editor(EditorMessage::ArtifactDescribed {
            instance: EditorInstanceId(3),
            description: ArtifactDescription::Unreadable("settings are unreadable".to_owned()),
        }),
        Message::Editor(EditorMessage::ArtifactEdited {
            instance: EditorInstanceId(3),
            data: b"{\"scale\":8}".to_vec(),
        }),
        Message::Editor(EditorMessage::RegenerateArtifact {
            instance: EditorInstanceId(3),
            data: b"{\"scale\":8}".to_vec(),
        }),
        Message::Editor(EditorMessage::ArtifactRegenerated {
            instance: EditorInstanceId(3),
            outcome: RegenerationOutcome::Done,
        }),
        Message::Editor(EditorMessage::ArtifactRegenerated {
            instance: EditorInstanceId(3),
            outcome: RegenerationOutcome::Failed("the source block is gone".to_owned()),
        }),
    ];
    for message in messages {
        let frame = encode_frame(&message).expect("the message encodes");
        assert_eq!(decode_frame(&frame).expect("the frame decodes"), message);
    }
}
