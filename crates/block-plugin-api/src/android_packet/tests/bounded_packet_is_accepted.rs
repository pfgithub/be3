use super::*;

#[test]
fn bounded_packet_is_accepted() {
    assert!(matches!(
        frame_packet().validate(),
        Ok(Message::FrameReady(_))
    ));
    assert!(matches!(
        surface_packet().validate(),
        Ok(Message::Surface(_))
    ));
}
