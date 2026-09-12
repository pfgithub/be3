use super::*;

#[test]
fn touch_input_round_trips() {
    let message = Message::Input(InputBatch {
        screen: ScreenId(3),
        events: vec![InputEvent::Touch {
            device: 4,
            finger: 5,
            phase: TouchPhase::Move,
            x: 12.0,
            y: 24.0,
            force: Some(0.75),
        }],
    });

    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
