use crate::{
    AlphaMode, AttachmentDescriptor, AttachmentOwnership, AttachmentType, ColorFormat, ColorSpace,
    SurfaceDescriptor, SurfaceMechanism, SurfaceRole,
};
use std::fmt;

const DESCRIPTOR_VERSION: u16 = 1;
const DESCRIPTOR_LENGTH: usize = 22;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowsSurfaceDescriptor {
    pub adapter_luid: u64,
    pub texture_format: u32,
    pub initial_fence_value: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowsSurfaceError {
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

impl fmt::Display for WindowsSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for WindowsSurfaceError {}

impl WindowsSurfaceDescriptor {
    pub fn encode(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(DESCRIPTOR_LENGTH);
        bytes.extend_from_slice(&DESCRIPTOR_VERSION.to_be_bytes());
        bytes.extend_from_slice(&self.adapter_luid.to_be_bytes());
        bytes.extend_from_slice(&self.texture_format.to_be_bytes());
        bytes.extend_from_slice(&self.initial_fence_value.to_be_bytes());
        bytes
    }

    pub fn decode(surface: &SurfaceDescriptor) -> Result<Self, WindowsSurfaceError> {
        validate_surface(surface)?;
        let bytes: [u8; DESCRIPTOR_LENGTH] = surface
            .opaque
            .as_slice()
            .try_into()
            .map_err(|_| WindowsSurfaceError::MalformedDescriptor)?;
        if u16::from_be_bytes(bytes[0..2].try_into().unwrap()) != DESCRIPTOR_VERSION {
            return Err(WindowsSurfaceError::MalformedDescriptor);
        }
        let descriptor = Self {
            adapter_luid: u64::from_be_bytes(bytes[2..10].try_into().unwrap()),
            texture_format: u32::from_be_bytes(bytes[10..14].try_into().unwrap()),
            initial_fence_value: u64::from_be_bytes(bytes[14..22].try_into().unwrap()),
        };
        if descriptor.adapter_luid == 0 || descriptor.texture_format == 0 {
            return Err(WindowsSurfaceError::MalformedDescriptor);
        }
        Ok(descriptor)
    }

    pub fn surface(
        self,
        request_id: u64,
        generation: u64,
        role: SurfaceRole,
        width: u32,
        height: u32,
    ) -> SurfaceDescriptor {
        SurfaceDescriptor {
            request_id,
            generation,
            role,
            mechanism: SurfaceMechanism::WindowsDxgi,
            width,
            height,
            format: ColorFormat::Bgra8Srgb,
            color_space: ColorSpace::Srgb,
            alpha_mode: AlphaMode::Premultiplied,
            attachments: vec![
                AttachmentDescriptor {
                    attachment_type: AttachmentType::Image,
                    ownership: AttachmentOwnership::Transferred,
                },
                AttachmentDescriptor {
                    attachment_type: AttachmentType::Synchronization,
                    ownership: AttachmentOwnership::Transferred,
                },
            ],
            opaque: self.encode(),
        }
    }
}

fn validate_surface(surface: &SurfaceDescriptor) -> Result<(), WindowsSurfaceError> {
    if surface.mechanism != SurfaceMechanism::WindowsDxgi {
        return Err(WindowsSurfaceError::WrongMechanism);
    }
    if surface.width == 0 || surface.height == 0 {
        return Err(WindowsSurfaceError::InvalidDimensions);
    }
    if surface.format != ColorFormat::Bgra8Srgb {
        return Err(WindowsSurfaceError::UnsupportedFormat);
    }
    if surface.color_space != ColorSpace::Srgb {
        return Err(WindowsSurfaceError::UnsupportedColorSpace);
    }
    if surface.alpha_mode != AlphaMode::Premultiplied {
        return Err(WindowsSurfaceError::UnsupportedAlphaMode);
    }
    if surface.attachments
        != [
            AttachmentDescriptor {
                attachment_type: AttachmentType::Image,
                ownership: AttachmentOwnership::Transferred,
            },
            AttachmentDescriptor {
                attachment_type: AttachmentType::Synchronization,
                ownership: AttachmentOwnership::Transferred,
            },
        ]
    {
        return Err(WindowsSurfaceError::InvalidAttachments);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsSurfaceState {
    Empty,
    Ready,
    Failed,
    Released,
}

#[derive(Debug)]
pub struct WindowsSurfaceLifecycle {
    state: WindowsSurfaceState,
    generation: u64,
    synchronization_value: u64,
}

impl Default for WindowsSurfaceLifecycle {
    fn default() -> Self {
        Self {
            state: WindowsSurfaceState::Empty,
            generation: 0,
            synchronization_value: 0,
        }
    }
}

impl WindowsSurfaceLifecycle {
    pub fn state(&self) -> WindowsSurfaceState {
        self.state
    }

    pub fn replace(
        &mut self,
        surface: &SurfaceDescriptor,
    ) -> Result<WindowsSurfaceDescriptor, WindowsSurfaceError> {
        if self.state == WindowsSurfaceState::Released {
            return Err(WindowsSurfaceError::Released);
        }
        if surface.generation <= self.generation {
            return Err(WindowsSurfaceError::InvalidGeneration);
        }
        let descriptor = WindowsSurfaceDescriptor::decode(surface)?;
        self.generation = surface.generation;
        self.synchronization_value = descriptor.initial_fence_value;
        self.state = WindowsSurfaceState::Ready;
        Ok(descriptor)
    }

    pub fn frame_ready(
        &mut self,
        generation: u64,
        synchronization_value: u64,
    ) -> Result<(), WindowsSurfaceError> {
        if self.state != WindowsSurfaceState::Ready {
            return Err(WindowsSurfaceError::Released);
        }
        if generation != self.generation {
            return Err(WindowsSurfaceError::InvalidGeneration);
        }
        if synchronization_value <= self.synchronization_value {
            return Err(WindowsSurfaceError::SynchronizationRegression);
        }
        self.synchronization_value = synchronization_value;
        Ok(())
    }

    pub fn fail(&mut self, error: WindowsSurfaceError) -> WindowsSurfaceError {
        self.state = WindowsSurfaceState::Failed;
        error
    }

    pub fn release(&mut self) {
        self.state = WindowsSurfaceState::Released;
    }
}

#[cfg(test)]
mod tests;
