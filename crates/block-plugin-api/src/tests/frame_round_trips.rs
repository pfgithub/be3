use super::*;

#[test]
fn frame_round_trips() {
    let message = hello();
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
