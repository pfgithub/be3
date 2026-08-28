use crate::{
    AlphaMode, AttachmentDescriptor, AttachmentOwnership, AttachmentType, ColorFormat, ColorSpace,
    SurfaceDescriptor, SurfaceMechanism, SurfaceRole, MAX_SURFACE_BUFFERS,
};
use std::fmt;

const DESCRIPTOR_VERSION: u16 = 1;
const HEADER_LENGTH: usize = 35;
const BUFFER_LENGTH: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinuxGraphicsBackend {
    Vulkan,
    Gl,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinuxSurfaceBuffer {
    pub offset: u32,
    pub stride: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinuxSurfaceDescriptor {
    pub drm_format: u32,
    pub modifier: u64,
    pub synchronization_value: u32,
    pub device: [u8; 16],
    pub buffers: Vec<LinuxSurfaceBuffer>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinuxSurfaceError {
    WrongMechanism,
    InvalidDimensions,
    UnsupportedBackend,
    UnsupportedFormat,
    UnsupportedColorSpace,
    UnsupportedAlphaMode,
    InvalidAttachments,
    MalformedDescriptor,
    InvalidGeneration,
    SynchronizationRegression,
    SynchronizationFailed,
    DeviceLost,
    Released,
}

impl fmt::Display for LinuxSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LinuxSurfaceError {}

impl LinuxSurfaceDescriptor {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_LENGTH + self.buffers.len() * BUFFER_LENGTH);
        bytes.extend_from_slice(&DESCRIPTOR_VERSION.to_be_bytes());
        bytes.extend_from_slice(&self.drm_format.to_be_bytes());
        bytes.extend_from_slice(&self.modifier.to_be_bytes());
        bytes.extend_from_slice(&self.synchronization_value.to_be_bytes());
        bytes.extend_from_slice(&self.device);
        bytes.push(self.buffers.len() as u8);
        for buffer in &self.buffers {
            bytes.extend_from_slice(&buffer.offset.to_be_bytes());
            bytes.extend_from_slice(&buffer.stride.to_be_bytes());
        }
        bytes
    }

    pub fn decode(surface: &SurfaceDescriptor) -> Result<Self, LinuxSurfaceError> {
        validate_surface(surface)?;
        if surface.opaque.len() < HEADER_LENGTH
            || u16::from_be_bytes(surface.opaque[0..2].try_into().unwrap()) != DESCRIPTOR_VERSION
        {
            return Err(LinuxSurfaceError::MalformedDescriptor);
        }
        let buffer_count = surface.opaque[HEADER_LENGTH - 1] as usize;
        if buffer_count == 0
            || buffer_count > MAX_SURFACE_BUFFERS
            || surface.opaque.len() != HEADER_LENGTH + buffer_count * BUFFER_LENGTH
            || surface.attachments.len() != buffer_count
        {
            return Err(LinuxSurfaceError::MalformedDescriptor);
        }
        let mut buffers = Vec::with_capacity(buffer_count);
        for bytes in surface.opaque[HEADER_LENGTH..].chunks_exact(BUFFER_LENGTH) {
            let buffer = LinuxSurfaceBuffer {
                offset: u32::from_be_bytes(bytes[0..4].try_into().unwrap()),
                stride: u32::from_be_bytes(bytes[4..8].try_into().unwrap()),
            };
            if buffer.stride == 0 {
                return Err(LinuxSurfaceError::MalformedDescriptor);
            }
            buffers.push(buffer);
        }
        let descriptor = Self {
            drm_format: u32::from_be_bytes(surface.opaque[2..6].try_into().unwrap()),
            modifier: u64::from_be_bytes(surface.opaque[6..14].try_into().unwrap()),
            synchronization_value: u32::from_be_bytes(surface.opaque[14..18].try_into().unwrap()),
            device: surface.opaque[18..34].try_into().unwrap(),
            buffers,
        };
        if descriptor.drm_format == 0 {
            return Err(LinuxSurfaceError::MalformedDescriptor);
        }
        Ok(descriptor)
    }

    pub fn surface(
        &self,
        request_id: u64,
        generation: u64,
        role: SurfaceRole,
        width: u32,
        height: u32,
    ) -> SurfaceDescriptor {
        let attachments = vec![
            AttachmentDescriptor {
                attachment_type: AttachmentType::Image,
                ownership: AttachmentOwnership::Transferred,
            };
            self.buffers.len()
        ];
        SurfaceDescriptor {
            request_id,
            generation,
            role,
            mechanism: SurfaceMechanism::LinuxDmaBuf,
            width,
            height,
            format: ColorFormat::Bgra8Srgb,
            color_space: ColorSpace::Srgb,
            alpha_mode: AlphaMode::Premultiplied,
            attachments,
            opaque: self.encode(),
        }
    }

    pub fn supports_backend(backend: LinuxGraphicsBackend) -> Result<(), LinuxSurfaceError> {
        match backend {
            LinuxGraphicsBackend::Vulkan | LinuxGraphicsBackend::Gl => Ok(()),
            LinuxGraphicsBackend::Other => Err(LinuxSurfaceError::UnsupportedBackend),
        }
    }
}

fn validate_surface(surface: &SurfaceDescriptor) -> Result<(), LinuxSurfaceError> {
    if surface.mechanism != SurfaceMechanism::LinuxDmaBuf {
        return Err(LinuxSurfaceError::WrongMechanism);
    }
    if surface.width == 0 || surface.height == 0 {
        return Err(LinuxSurfaceError::InvalidDimensions);
    }
    if surface.format != ColorFormat::Bgra8Srgb {
        return Err(LinuxSurfaceError::UnsupportedFormat);
    }
    if surface.color_space != ColorSpace::Srgb {
        return Err(LinuxSurfaceError::UnsupportedColorSpace);
    }
    if surface.alpha_mode != AlphaMode::Premultiplied {
        return Err(LinuxSurfaceError::UnsupportedAlphaMode);
    }
    if surface.attachments.is_empty()
        || surface.attachments.iter().any(|attachment| {
            attachment.attachment_type != AttachmentType::Image
                || attachment.ownership != AttachmentOwnership::Transferred
        })
    {
        return Err(LinuxSurfaceError::InvalidAttachments);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinuxSurfaceState {
    Empty,
    Ready,
    Failed,
    Released,
}

#[derive(Debug)]
pub struct LinuxSurfaceLifecycle {
    state: LinuxSurfaceState,
    generation: u64,
    synchronization_value: u32,
}

impl Default for LinuxSurfaceLifecycle {
    fn default() -> Self {
        Self {
            state: LinuxSurfaceState::Empty,
            generation: 0,
            synchronization_value: 0,
        }
    }
}

impl LinuxSurfaceLifecycle {
    pub fn state(&self) -> LinuxSurfaceState {
        self.state
    }

    pub fn replace(
        &mut self,
        surface: &SurfaceDescriptor,
    ) -> Result<LinuxSurfaceDescriptor, LinuxSurfaceError> {
        if self.state == LinuxSurfaceState::Released {
            return Err(LinuxSurfaceError::Released);
        }
        if surface.generation <= self.generation {
            return Err(LinuxSurfaceError::InvalidGeneration);
        }
        let descriptor = LinuxSurfaceDescriptor::decode(surface)?;
        self.generation = surface.generation;
        self.synchronization_value = descriptor.synchronization_value;
        self.state = LinuxSurfaceState::Ready;
        Ok(descriptor)
    }

    pub fn frame_ready(
        &mut self,
        generation: u64,
        synchronization_value: u32,
    ) -> Result<(), LinuxSurfaceError> {
        if self.state != LinuxSurfaceState::Ready {
            return Err(LinuxSurfaceError::Released);
        }
        if generation != self.generation {
            return Err(LinuxSurfaceError::InvalidGeneration);
        }
        if synchronization_value <= self.synchronization_value {
            return Err(LinuxSurfaceError::SynchronizationRegression);
        }
        self.synchronization_value = synchronization_value;
        Ok(())
    }

    pub fn fail(&mut self, error: LinuxSurfaceError) -> LinuxSurfaceError {
        self.state = LinuxSurfaceState::Failed;
        error
    }

    pub fn release(&mut self) {
        self.state = LinuxSurfaceState::Released;
    }
}

#[cfg(test)]
mod tests;
