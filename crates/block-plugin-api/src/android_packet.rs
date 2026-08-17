use crate::{
    decode_frame, validate_attachments, AttachmentError, AttachmentType, Message, MAX_FRAME_BYTES,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AndroidPacket {
    pub frame: Vec<u8>,
    pub has_hardware_buffer: bool,
    pub fence_descriptor_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AndroidPacketError {
    FrameTooLarge,
    MalformedFrame,
    Attachment(AttachmentError),
}

impl AndroidPacket {
    pub fn validate(&self) -> Result<Message, AndroidPacketError> {
        if self.frame.len() > MAX_FRAME_BYTES + 4 {
            return Err(AndroidPacketError::FrameTooLarge);
        }
        let message = decode_frame(&self.frame).map_err(|_| AndroidPacketError::MalformedFrame)?;
        let expected = match &message {
            Message::Surface(surface) => surface.attachments.as_slice(),
            Message::FrameReady(frame) => frame.attachments.as_slice(),
            _ => &[],
        };
        let mut received =
            Vec::with_capacity(usize::from(self.has_hardware_buffer) + self.fence_descriptor_count);
        if self.has_hardware_buffer {
            received.push(AttachmentType::Image);
        }
        received.extend(std::iter::repeat_n(
            AttachmentType::Synchronization,
            self.fence_descriptor_count,
        ));
        validate_attachments(expected, &received).map_err(AndroidPacketError::Attachment)?;
        Ok(message)
    }
}

#[cfg(test)]
mod tests;
