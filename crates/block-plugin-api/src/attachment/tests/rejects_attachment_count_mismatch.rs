use super::*;

#[test]
fn rejects_attachment_count_mismatch() {
    let expected = [AttachmentDescriptor {
        attachment_type: AttachmentType::Image,
        ownership: AttachmentOwnership::Transferred,
    }];
    assert_eq!(
        validate_attachments(&expected, &[]),
        Err(AttachmentError::CountMismatch {
            expected: 1,
            received: 0,
        })
    );
}
