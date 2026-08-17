use super::*;

fn surface(generation: u64) -> SurfaceDescriptor {
    AndroidSurfaceDescriptor::rgba8_srgb().surface(1, generation, 64, 32)
}

fn frame(generation: u64, synchronization_value: u64) -> FrameReady {
    FrameReady {
        generation,
        damage: Vec::new(),
        synchronization_value,
        attachments: vec![AttachmentDescriptor {
            attachment_type: AttachmentType::Synchronization,
            ownership: AttachmentOwnership::Transferred,
        }],
    }
}

mod attachment_mismatch;
mod descriptor_round_trips;
mod failure_changes_state;
mod malformed_descriptor_is_rejected;
mod release_is_terminal;
mod stale_generation_is_rejected;
mod synchronization_regression_is_rejected;
