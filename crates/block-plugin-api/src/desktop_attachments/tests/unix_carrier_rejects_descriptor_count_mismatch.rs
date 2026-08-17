use crate::{
    desktop_attachments::{CarrierError, UnixAttachmentCarrier},
    AttachmentError, Message,
};
use std::os::unix::net::UnixStream;

#[test]
fn unix_carrier_rejects_descriptor_count_mismatch() {
    let (stream, _) = UnixStream::pair().unwrap();
    let mut carrier = UnixAttachmentCarrier::new(stream);
    assert!(matches!(
        carrier.send(&Message::Ping { nonce: 1 }, &[0]),
        Err(CarrierError::Attachments(AttachmentError::CountMismatch {
            expected: 0,
            received: 1,
        }))
    ));
}
