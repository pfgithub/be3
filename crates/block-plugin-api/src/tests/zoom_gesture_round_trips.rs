use super::*;

#[test]
fn zoom_gesture_round_trips() {
    let message = Message::Input(InputBatch {
        screen: ScreenId(3),
        events: vec![InputEvent::Zoom { factor: 1.25 }],
    });

    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
