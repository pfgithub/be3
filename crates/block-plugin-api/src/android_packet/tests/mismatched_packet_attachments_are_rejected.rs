use super::*;

#[test]
fn mismatched_packet_attachments_are_rejected() {
    let mut packet = frame_packet();
    packet.fence_descriptor_count = 0;
    assert!(matches!(
        packet.validate(),
        Err(AndroidPacketError::Attachment(_))
    ));
}
