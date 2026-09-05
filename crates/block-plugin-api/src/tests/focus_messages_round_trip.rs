use super::*;

#[test]
fn focus_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::Focused {
            instance: EditorInstanceId(4),
            block_id: Some([7; 16]),
            block_type: [8; 16],
            via: vec![[9; 16], [10; 16]],
        }),
        Message::Editor(EditorMessage::Focused {
            instance: EditorInstanceId(4),
            block_id: None,
            block_type: [0; 16],
            via: Vec::new(),
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
