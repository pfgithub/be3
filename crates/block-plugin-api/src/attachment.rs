use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_ATTACHMENTS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttachmentType {
    Image,
    Synchronization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttachmentOwnership {
    Borrowed,
    Transferred,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentDescriptor {
    pub attachment_type: AttachmentType,
    pub ownership: AttachmentOwnership,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttachmentError {
    TooMany {
        count: usize,
        maximum: usize,
    },
    CountMismatch {
        expected: usize,
        received: usize,
    },
    TypeMismatch {
        index: usize,
        expected: AttachmentType,
        received: AttachmentType,
    },
}

impl fmt::Display for AttachmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AttachmentError {}

pub fn validate_attachments(
    expected: &[AttachmentDescriptor],
    received: &[AttachmentType],
) -> Result<(), AttachmentError> {
    if expected.len() > MAX_ATTACHMENTS {
        return Err(AttachmentError::TooMany {
            count: expected.len(),
            maximum: MAX_ATTACHMENTS,
        });
    }
    if expected.len() != received.len() {
        return Err(AttachmentError::CountMismatch {
            expected: expected.len(),
            received: received.len(),
        });
    }
    for (index, (expected, received)) in expected.iter().zip(received).enumerate() {
        if expected.attachment_type != *received {
            return Err(AttachmentError::TypeMismatch {
                index,
                expected: expected.attachment_type,
                received: *received,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
