use super::*;

#[test]
fn replacing_a_child_round_trips() {
    for message in [
        Message::Editor(EditorMessage::ReplaceChild {
            instance: EditorInstanceId(3),
            request_id: 9,
            old: [1; 16],
            new: [2; 16],
        }),
        Message::Editor(EditorMessage::ChildReplaced {
            instance: EditorInstanceId(3),
            request_id: 9,
            replaced: true,
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
