use super::*;

#[test]
fn copied_text_round_trips() {
    let message = Message::Editor(EditorMessage::CopyText {
        instance: EditorInstanceId(5),
        text: "hello".to_owned(),
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );

    let oversized = Message::Editor(EditorMessage::CopyText {
        instance: EditorInstanceId(5),
        text: "x".repeat(MAX_STRING_BYTES + 1),
    });
    assert_eq!(
        encode_frame(&oversized),
        Err(DecodeError::LimitExceeded("string"))
    );
}
