use bincode::Options;
use serde::{Deserialize, Serialize};
use std::fmt;

mod android_packet;
mod android_surface;
mod attachment;
#[cfg(any(unix, windows))]
pub mod desktop_attachments;
mod linux_surface;
mod macos_surface;
mod session;
mod windows_surface;

pub use android_packet::{AndroidPacket, AndroidPacketError};
pub use android_surface::{
    AndroidSurfaceDescriptor, AndroidSurfaceError, AndroidSurfaceLifecycle, AndroidSurfaceState,
    ANDROID_HARDWARE_BUFFER_FORMAT_R8G8B8A8_UNORM, ANDROID_HARDWARE_BUFFER_USAGE_GPU_COLOR_OUTPUT,
    ANDROID_HARDWARE_BUFFER_USAGE_GPU_SAMPLED_IMAGE,
};
pub use attachment::{
    validate_attachments, AttachmentDescriptor, AttachmentError, AttachmentOwnership,
    AttachmentType, MAX_ATTACHMENTS,
};
pub use linux_surface::{
    LinuxGraphicsBackend, LinuxSurfaceDescriptor, LinuxSurfaceError, LinuxSurfaceLifecycle,
    LinuxSurfacePlane, LinuxSurfaceState,
};
pub use macos_surface::{
    MacOsSurfaceDescriptor, MacOsSurfaceError, MacOsSurfaceLifecycle, MacOsSurfaceState,
};
pub use session::{HostSession, QueueError, SessionFailure, SessionState};
pub use windows_surface::{
    WindowsSurfaceDescriptor, WindowsSurfaceError, WindowsSurfaceLifecycle, WindowsSurfaceState,
};

pub const PROTOCOL_VERSION: u16 = 2;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_COLLECTION_ITEMS: usize = 1024;
pub const MAX_STRING_BYTES: usize = 16 * 1024;
pub const MAX_OPAQUE_DESCRIPTOR_BYTES: usize = 64 * 1024;
pub const MAX_QUEUED_MESSAGES: usize = 256;
pub const REQUEST_TIMEOUT_MILLISECONDS: u64 = 5_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Message {
    Hello(Hello),
    HelloAccepted(HelloAccepted),
    HelloRejected(ProtocolError),
    CreateViewport(CreateViewport),
    ResizeViewport(ViewportMetrics),
    Input(InputBatch),
    SurfaceCapabilities(SurfaceCapabilities),
    Surface(SurfaceDescriptor),
    FrameReady(FrameReady),
    FramePresented { generation: u64 },
    Acknowledged { request_id: u64 },
    Ping { nonce: u64 },
    Pong { nonce: u64 },
    Error(ProtocolError),
    Shutdown,
    ShutdownAcknowledged,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub minimum_version: u16,
    pub maximum_version: u16,
    pub plugin: PluginIdentity,
    pub capabilities: Vec<Capability>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloAccepted {
    pub version: u16,
    pub host_name: String,
    pub capabilities: Vec<Capability>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginIdentity {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capability {
    Input,
    Lifecycle,
    Surface(SurfaceMechanism),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceMechanism {
    WebExternalImage,
    MacOsIoSurface,
    WindowsDxgi,
    LinuxDmaBuf,
    AndroidHardwareBuffer,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateViewport {
    pub request_id: u64,
    pub metrics: ViewportMetrics,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewportMetrics {
    pub logical_width: f32,
    pub logical_height: f32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub scale_factor: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputBatch {
    pub viewport_request_id: u64,
    pub events: Vec<InputEvent>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    PointerMoved {
        x: f32,
        y: f32,
    },
    PointerButton {
        button: PointerButton,
        pressed: bool,
        x: f32,
        y: f32,
    },
    Wheel {
        x: f32,
        y: f32,
        unit: WheelUnit,
    },
    Key {
        physical: PhysicalKey,
        logical: String,
        pressed: bool,
        repeat: bool,
    },
    Text(String),
    Modifiers(Modifiers),
    Focus(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WheelUnit {
    Pixels,
    Lines,
    Pages,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicalKey {
    Code(u32),
    Unidentified,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifiers {
    pub alt: bool,
    pub control: bool,
    pub shift: bool,
    pub command: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceCapabilities {
    pub request_id: u64,
    pub mechanisms: Vec<SurfaceMechanism>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceDescriptor {
    pub request_id: u64,
    pub generation: u64,
    pub mechanism: SurfaceMechanism,
    pub width: u32,
    pub height: u32,
    pub format: ColorFormat,
    pub color_space: ColorSpace,
    pub alpha_mode: AlphaMode,
    pub attachments: Vec<AttachmentDescriptor>,
    pub opaque: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorFormat {
    Rgba8Srgb,
    Bgra8Srgb,
    Rgba16Float,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorSpace {
    Srgb,
    DisplayP3,
    ExtendedSrgb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlphaMode {
    Opaque,
    Premultiplied,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameReady {
    pub generation: u64,
    pub damage: Vec<DamageRect>,
    pub synchronization_value: u64,
    pub attachments: Vec<AttachmentDescriptor>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolError {
    pub request_id: Option<u64>,
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    UnsupportedVersion,
    UnsupportedCapability,
    InvalidMessage,
    InvalidState,
    Timeout,
    Internal,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    FrameTooLarge { length: usize, maximum: usize },
    TruncatedFrame { expected: usize, available: usize },
    MalformedPayload,
    LimitExceeded(&'static str),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DecodeError {}

pub fn encode_frame(message: &Message) -> Result<Vec<u8>, DecodeError> {
    validate(message)?;
    let payload = codec()
        .serialize(message)
        .map_err(|_| DecodeError::MalformedPayload)?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(DecodeError::FrameTooLarge {
            length: payload.len(),
            maximum: MAX_FRAME_BYTES,
        });
    }
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn decode_frame(frame: &[u8]) -> Result<Message, DecodeError> {
    if frame.len() < 4 {
        return Err(DecodeError::TruncatedFrame {
            expected: 4,
            available: frame.len(),
        });
    }
    let length = u32::from_be_bytes(frame[..4].try_into().unwrap()) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(DecodeError::FrameTooLarge {
            length,
            maximum: MAX_FRAME_BYTES,
        });
    }
    if frame.len() != length + 4 {
        return Err(DecodeError::TruncatedFrame {
            expected: length + 4,
            available: frame.len(),
        });
    }
    let message = codec()
        .deserialize(&frame[4..])
        .map_err(|_| DecodeError::MalformedPayload)?;
    validate(&message)?;
    Ok(message)
}

fn validate(message: &Message) -> Result<(), DecodeError> {
    match message {
        Message::Hello(value) => {
            strings([&value.plugin.id, &value.plugin.name, &value.plugin.version])?;
            collection(value.capabilities.len())
        }
        Message::HelloAccepted(value) => {
            string(&value.host_name)?;
            collection(value.capabilities.len())
        }
        Message::HelloRejected(value) | Message::Error(value) => string(&value.message),
        Message::Input(value) => {
            collection(value.events.len())?;
            for event in &value.events {
                if let InputEvent::Key { logical, .. } | InputEvent::Text(logical) = event {
                    string(logical)?;
                }
            }
            Ok(())
        }
        Message::SurfaceCapabilities(value) => collection(value.mechanisms.len()),
        Message::Surface(value) => {
            if value.opaque.len() > MAX_OPAQUE_DESCRIPTOR_BYTES {
                return Err(DecodeError::LimitExceeded("surface descriptor"));
            }
            if value.attachments.len() > MAX_ATTACHMENTS {
                return Err(DecodeError::LimitExceeded("surface attachments"));
            }
            Ok(())
        }
        Message::FrameReady(value) => {
            collection(value.damage.len())?;
            if value.attachments.len() > MAX_ATTACHMENTS {
                return Err(DecodeError::LimitExceeded("frame attachments"));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn collection(length: usize) -> Result<(), DecodeError> {
    if length > MAX_COLLECTION_ITEMS {
        Err(DecodeError::LimitExceeded("collection"))
    } else {
        Ok(())
    }
}

fn string(value: &str) -> Result<(), DecodeError> {
    if value.len() > MAX_STRING_BYTES {
        Err(DecodeError::LimitExceeded("string"))
    } else {
        Ok(())
    }
}

fn strings<'a>(values: impl IntoIterator<Item = &'a String>) -> Result<(), DecodeError> {
    for value in values {
        string(value)?;
    }
    Ok(())
}

fn codec() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(MAX_FRAME_BYTES as u64)
        .reject_trailing_bytes()
}

#[cfg(test)]
mod tests;
