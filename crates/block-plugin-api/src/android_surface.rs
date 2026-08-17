use crate::{
    AlphaMode, AttachmentDescriptor, AttachmentOwnership, AttachmentType, ColorFormat, ColorSpace,
    FrameReady, SurfaceDescriptor, SurfaceMechanism,
};
use std::fmt;

const DESCRIPTOR_VERSION: u16 = 1;
const DESCRIPTOR_LENGTH: usize = 14;

pub const ANDROID_HARDWARE_BUFFER_FORMAT_R8G8B8A8_UNORM: u32 = 1;
pub const ANDROID_HARDWARE_BUFFER_USAGE_GPU_SAMPLED_IMAGE: u64 = 1 << 8;
pub const ANDROID_HARDWARE_BUFFER_USAGE_GPU_COLOR_OUTPUT: u64 = 1 << 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AndroidSurfaceDescriptor {
    pub hardware_buffer_format: u32,
    pub hardware_buffer_usage: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AndroidSurfaceError {
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

impl fmt::Display for AndroidSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AndroidSurfaceError {}

impl AndroidSurfaceDescriptor {
    pub fn rgba8_srgb() -> Self {
        Self {
            hardware_buffer_format: ANDROID_HARDWARE_BUFFER_FORMAT_R8G8B8A8_UNORM,
            hardware_buffer_usage: ANDROID_HARDWARE_BUFFER_USAGE_GPU_SAMPLED_IMAGE
                | ANDROID_HARDWARE_BUFFER_USAGE_GPU_COLOR_OUTPUT,
        }
    }

    pub fn encode(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(DESCRIPTOR_LENGTH);
        bytes.extend_from_slice(&DESCRIPTOR_VERSION.to_be_bytes());
        bytes.extend_from_slice(&self.hardware_buffer_format.to_be_bytes());
        bytes.extend_from_slice(&self.hardware_buffer_usage.to_be_bytes());
        bytes
    }

    pub fn decode(surface: &SurfaceDescriptor) -> Result<Self, AndroidSurfaceError> {
        validate_surface(surface)?;
        let bytes: [u8; DESCRIPTOR_LENGTH] = surface
            .opaque
            .as_slice()
            .try_into()
            .map_err(|_| AndroidSurfaceError::MalformedDescriptor)?;
        if u16::from_be_bytes(bytes[0..2].try_into().unwrap()) != DESCRIPTOR_VERSION {
            return Err(AndroidSurfaceError::MalformedDescriptor);
        }
        let descriptor = Self {
            hardware_buffer_format: u32::from_be_bytes(bytes[2..6].try_into().unwrap()),
            hardware_buffer_usage: u64::from_be_bytes(bytes[6..14].try_into().unwrap()),
        };
        if descriptor != Self::rgba8_srgb() {
            return Err(AndroidSurfaceError::MalformedDescriptor);
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
            mechanism: SurfaceMechanism::AndroidHardwareBuffer,
            width,
            height,
            format: ColorFormat::Rgba8Srgb,
            color_space: ColorSpace::Srgb,
            alpha_mode: AlphaMode::Premultiplied,
            attachments: vec![AttachmentDescriptor {
                attachment_type: AttachmentType::Image,
                ownership: AttachmentOwnership::Transferred,
            }],
            opaque: self.encode(),
        }
    }
}

fn validate_surface(surface: &SurfaceDescriptor) -> Result<(), AndroidSurfaceError> {
    if surface.mechanism != SurfaceMechanism::AndroidHardwareBuffer {
        return Err(AndroidSurfaceError::WrongMechanism);
    }
    if surface.width == 0 || surface.height == 0 {
        return Err(AndroidSurfaceError::InvalidDimensions);
    }
    if surface.format != ColorFormat::Rgba8Srgb {
        return Err(AndroidSurfaceError::UnsupportedFormat);
    }
    if surface.color_space != ColorSpace::Srgb {
        return Err(AndroidSurfaceError::UnsupportedColorSpace);
    }
    if surface.alpha_mode != AlphaMode::Premultiplied {
        return Err(AndroidSurfaceError::UnsupportedAlphaMode);
    }
    if surface.attachments
        != [AttachmentDescriptor {
            attachment_type: AttachmentType::Image,
            ownership: AttachmentOwnership::Transferred,
        }]
    {
        return Err(AndroidSurfaceError::InvalidAttachments);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AndroidSurfaceState {
    Empty,
    Ready,
    Failed,
    Released,
}

#[derive(Debug)]
pub struct AndroidSurfaceLifecycle {
    state: AndroidSurfaceState,
    generation: u64,
    synchronization_value: u64,
}

impl Default for AndroidSurfaceLifecycle {
    fn default() -> Self {
        Self {
            state: AndroidSurfaceState::Empty,
            generation: 0,
            synchronization_value: 0,
        }
    }
}

impl AndroidSurfaceLifecycle {
    pub fn state(&self) -> AndroidSurfaceState {
        self.state
    }

    pub fn replace(
        &mut self,
        surface: &SurfaceDescriptor,
    ) -> Result<AndroidSurfaceDescriptor, AndroidSurfaceError> {
        if self.state == AndroidSurfaceState::Released {
            return Err(AndroidSurfaceError::Released);
        }
        if surface.generation <= self.generation {
            return Err(AndroidSurfaceError::InvalidGeneration);
        }
        let descriptor = AndroidSurfaceDescriptor::decode(surface)?;
        self.generation = surface.generation;
        self.synchronization_value = 0;
        self.state = AndroidSurfaceState::Ready;
        Ok(descriptor)
    }

    pub fn frame_ready(&mut self, frame: &FrameReady) -> Result<(), AndroidSurfaceError> {
        if self.state != AndroidSurfaceState::Ready {
            return Err(AndroidSurfaceError::Released);
        }
        if frame.generation != self.generation {
            return Err(AndroidSurfaceError::InvalidGeneration);
        }
        if frame.attachments
            != [AttachmentDescriptor {
                attachment_type: AttachmentType::Synchronization,
                ownership: AttachmentOwnership::Transferred,
            }]
        {
            return Err(AndroidSurfaceError::InvalidAttachments);
        }
        if frame.synchronization_value <= self.synchronization_value {
            return Err(AndroidSurfaceError::SynchronizationRegression);
        }
        self.synchronization_value = frame.synchronization_value;
        Ok(())
    }

    pub fn fail(&mut self, error: AndroidSurfaceError) -> AndroidSurfaceError {
        self.state = AndroidSurfaceState::Failed;
        error
    }
    pub fn release(&mut self) {
        self.state = AndroidSurfaceState::Released;
    }
}

#[cfg(test)]
mod tests;
