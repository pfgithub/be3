use super::super::WindowsAttachmentCarrier;

#[test]
fn windows_attachment_carrier_is_send() {
    fn assert_send<T: Send>() {}

    assert_send::<WindowsAttachmentCarrier>();
}
