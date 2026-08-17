use super::*;
use crate::{
    encode_frame, AndroidSurfaceDescriptor, AttachmentDescriptor, AttachmentOwnership, FrameReady,
};

mod bounded_packet_is_accepted;
mod mismatched_packet_attachments_are_rejected;

fn frame_packet() -> AndroidPacket {
    let message = Message::FrameReady(FrameReady {
        generation: 1,
        damage: Vec::new(),
        synchronization_value: 1,
        attachments: vec![AttachmentDescriptor {
            attachment_type: AttachmentType::Synchronization,
            ownership: AttachmentOwnership::Transferred,
        }],
    });
    AndroidPacket {
        frame: encode_frame(&message).unwrap(),
        has_hardware_buffer: false,
        fence_descriptor_count: 1,
    }
}

fn surface_packet() -> AndroidPacket {
    let message = Message::Surface(AndroidSurfaceDescriptor::rgba8_srgb().surface(1, 1, 8, 8));
    AndroidPacket {
        frame: encode_frame(&message).unwrap(),
        has_hardware_buffer: true,
        fence_descriptor_count: 0,
    }
}
