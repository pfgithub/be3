use super::*;

#[test]
fn cursor_round_trips() {
    let message = Message::Editor(EditorMessage::Cursor {
        instance: EditorInstanceId(7),
        region: EditorRegion::Main,
        cursor: CursorIcon::Crosshair,
    });
    let frame = encode_frame(&message).expect("the message encodes");
    assert_eq!(decode_frame(&frame).expect("the frame decodes"), message);
}
