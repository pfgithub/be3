use super::*;

#[test]
fn rejects_attachment_type_mismatch() {
    let expected = [AttachmentDescriptor {
        attachment_type: AttachmentType::Synchronization,
        ownership: AttachmentOwnership::Borrowed,
    }];
    assert_eq!(
        validate_attachments(&expected, &[AttachmentType::Image]),
        Err(AttachmentError::TypeMismatch {
            index: 0,
            expected: AttachmentType::Synchronization,
            received: AttachmentType::Image,
        })
    );
}
