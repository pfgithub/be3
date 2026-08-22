use super::*;

#[test]
fn rejects_artifact_settings_over_limit() {
    let message = Message::Editor(EditorMessage::RegenerateArtifact {
        instance: EditorInstanceId(1),
        data: vec![0; MAX_OPAQUE_DESCRIPTOR_BYTES + 1],
    });
    assert_eq!(
        encode_frame(&message),
        Err(DecodeError::LimitExceeded("artifact settings"))
    );
}
