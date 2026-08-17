use crate::{
    AlphaMode, AttachmentDescriptor, AttachmentOwnership, AttachmentType, ColorFormat, ColorSpace,
    SurfaceDescriptor, SurfaceMechanism,
};
use std::fmt;

const DESCRIPTOR_VERSION: u16 = 1;
const DESCRIPTOR_LENGTH: usize = 22;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MacOsSurfaceDescriptor {
    pub io_surface_id: u32,
    pub bytes_per_row: u32,
    pub pixel_format: u32,
    pub shared_event_value: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MacOsSurfaceError {
    WrongMechanism,
    InvalidDimensions,
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

impl fmt::Display for MacOsSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for MacOsSurfaceError {}

impl MacOsSurfaceDescriptor {
    pub fn encode(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(DESCRIPTOR_LENGTH);
        bytes.extend_from_slice(&DESCRIPTOR_VERSION.to_be_bytes());
        bytes.extend_from_slice(&self.io_surface_id.to_be_bytes());
        bytes.extend_from_slice(&self.bytes_per_row.to_be_bytes());
        bytes.extend_from_slice(&self.pixel_format.to_be_bytes());
        bytes.extend_from_slice(&self.shared_event_value.to_be_bytes());
        bytes
    }

    pub fn decode(surface: &SurfaceDescriptor) -> Result<Self, MacOsSurfaceError> {
        validate_surface(surface)?;
        let bytes: [u8; DESCRIPTOR_LENGTH] = surface
            .opaque
            .as_slice()
            .try_into()
            .map_err(|_| MacOsSurfaceError::MalformedDescriptor)?;
        if u16::from_be_bytes(bytes[0..2].try_into().unwrap()) != DESCRIPTOR_VERSION {
            return Err(MacOsSurfaceError::MalformedDescriptor);
        }
        let descriptor = Self {
            io_surface_id: u32::from_be_bytes(bytes[2..6].try_into().unwrap()),
            bytes_per_row: u32::from_be_bytes(bytes[6..10].try_into().unwrap()),
            pixel_format: u32::from_be_bytes(bytes[10..14].try_into().unwrap()),
            shared_event_value: u64::from_be_bytes(bytes[14..22].try_into().unwrap()),
        };
        if descriptor.io_surface_id == 0
            || descriptor.bytes_per_row < surface.width.saturating_mul(4)
            || descriptor.pixel_format == 0
        {
            return Err(MacOsSurfaceError::MalformedDescriptor);
        }
        Ok(descriptor)
    }

    pub fn surface(
        self,
        request_id: u64,
        generation: u64,
        width: u32,
        height: u32,
    ) -> SurfaceDescriptor {
        SurfaceDescriptor {
            request_id,
            generation,
            mechanism: SurfaceMechanism::MacOsIoSurface,
            width,
            height,
            format: ColorFormat::Bgra8Srgb,
            color_space: ColorSpace::Srgb,
            alpha_mode: AlphaMode::Premultiplied,
            attachments: vec![AttachmentDescriptor {
                attachment_type: AttachmentType::Synchronization,
                ownership: AttachmentOwnership::Transferred,
            }],
            opaque: self.encode(),
        }
    }
}

fn validate_surface(surface: &SurfaceDescriptor) -> Result<(), MacOsSurfaceError> {
    if surface.mechanism != SurfaceMechanism::MacOsIoSurface {
        return Err(MacOsSurfaceError::WrongMechanism);
    }
    if surface.width == 0 || surface.height == 0 {
        return Err(MacOsSurfaceError::InvalidDimensions);
    }
    if surface.format != ColorFormat::Bgra8Srgb {
        return Err(MacOsSurfaceError::UnsupportedFormat);
    }
    if !matches!(
        surface.color_space,
        ColorSpace::Srgb | ColorSpace::DisplayP3
    ) {
        return Err(MacOsSurfaceError::UnsupportedColorSpace);
    }
    if surface.alpha_mode != AlphaMode::Premultiplied {
        return Err(MacOsSurfaceError::UnsupportedAlphaMode);
    }
    if surface.attachments
        != [AttachmentDescriptor {
            attachment_type: AttachmentType::Synchronization,
            ownership: AttachmentOwnership::Transferred,
        }]
    {
        return Err(MacOsSurfaceError::InvalidAttachments);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacOsSurfaceState {
    Empty,
    Ready,
    Failed,
    Released,
}

#[derive(Debug)]
pub struct MacOsSurfaceLifecycle {
    state: MacOsSurfaceState,
    generation: u64,
    synchronization_value: u64,
}

impl Default for MacOsSurfaceLifecycle {
    fn default() -> Self {
        Self {
            state: MacOsSurfaceState::Empty,
            generation: 0,
            synchronization_value: 0,
        }
    }
}

impl MacOsSurfaceLifecycle {
    pub fn state(&self) -> MacOsSurfaceState {
        self.state
    }

    pub fn replace(
        &mut self,
        surface: &SurfaceDescriptor,
    ) -> Result<MacOsSurfaceDescriptor, MacOsSurfaceError> {
        if self.state == MacOsSurfaceState::Released {
            return Err(MacOsSurfaceError::Released);
        }
        if surface.generation <= self.generation {
            return Err(MacOsSurfaceError::InvalidGeneration);
        }
        let descriptor = MacOsSurfaceDescriptor::decode(surface)?;
        self.generation = surface.generation;
        self.synchronization_value = descriptor.shared_event_value;
        self.state = MacOsSurfaceState::Ready;
        Ok(descriptor)
    }

    pub fn frame_ready(
        &mut self,
        generation: u64,
        synchronization_value: u64,
    ) -> Result<(), MacOsSurfaceError> {
        if self.state != MacOsSurfaceState::Ready {
            return Err(MacOsSurfaceError::Released);
        }
        if generation != self.generation {
            return Err(MacOsSurfaceError::InvalidGeneration);
        }
        if synchronization_value <= self.synchronization_value {
            return Err(MacOsSurfaceError::SynchronizationRegression);
        }
        self.synchronization_value = synchronization_value;
        Ok(())
    }

    pub fn fail(&mut self, error: MacOsSurfaceError) -> MacOsSurfaceError {
        self.state = MacOsSurfaceState::Failed;
        error
    }

    pub fn release(&mut self) {
        self.state = MacOsSurfaceState::Released;
    }
}

#[cfg(test)]
mod tests;
