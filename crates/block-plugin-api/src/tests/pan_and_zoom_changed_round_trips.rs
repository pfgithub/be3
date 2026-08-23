use super::*;

#[test]
fn pan_and_zoom_changed_round_trips() {
    for owned in [false, true] {
        let message = Message::Editor(EditorMessage::PanAndZoomChanged {
            instance: EditorInstanceId(7),
            owned,
        });
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
