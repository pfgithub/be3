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

pub const PROTOCOL_VERSION: u16 = 3;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_COLLECTION_ITEMS: usize = 1024;
pub const MAX_STRING_BYTES: usize = 16 * 1024;
pub const MAX_OPAQUE_DESCRIPTOR_BYTES: usize = 64 * 1024;
pub const MAX_QUEUED_MESSAGES: usize = 256;
pub const REQUEST_TIMEOUT_MILLISECONDS: u64 = 5_000;
pub const MAX_BLOCK_PAYLOAD_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorInstanceId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub identity: PluginIdentity,
    pub block_type: [u8; 16],
    pub display_name: String,
    pub icon: String,
    pub creation: CreationMode,
    pub regions: Vec<EditorRegion>,
    pub entry_points: EntryPoints,
    pub surfaces: Vec<SurfaceMechanism>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreationMode {
    Immediate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorRegion {
    pub id: String,
    pub main: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryPoints {
    pub web: Option<String>,
    pub windows: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    Empty(&'static str),
    TooLong(&'static str),
    InvalidIdentity,
    InvalidRegions,
    MissingEntryPoint,
    MissingSurface,
}

impl PluginManifest {
    pub fn validate(&self) -> Result<(), ManifestError> {
        manifest_string("plugin id", &self.identity.id)?;
        manifest_string("plugin name", &self.identity.name)?;
        manifest_string("plugin version", &self.identity.version)?;
        manifest_string("display name", &self.display_name)?;
        manifest_string("icon", &self.icon)?;
        if !self
            .identity
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            return Err(ManifestError::InvalidIdentity);
        }
        if self.regions.len() != 1 || !self.regions[0].main {
            return Err(ManifestError::InvalidRegions);
        }
        manifest_string("region id", &self.regions[0].id)?;
        if self.entry_points.web.is_none() && self.entry_points.windows.is_none() {
            return Err(ManifestError::MissingEntryPoint);
        }
        for entry in [&self.entry_points.web, &self.entry_points.windows]
            .into_iter()
            .flatten()
        {
            manifest_string("entry point", entry)?;
        }
        let web_valid = self.entry_points.web.is_none()
            || self.surfaces.contains(&SurfaceMechanism::WebExternalImage);
        let windows_valid = self.entry_points.windows.is_none()
            || self.surfaces.contains(&SurfaceMechanism::WindowsDxgi);
        if !web_valid || !windows_valid {
            return Err(ManifestError::MissingSurface);
        }
        Ok(())
    }
}

fn manifest_string(field: &'static str, value: &str) -> Result<(), ManifestError> {
    if value.is_empty() {
        Err(ManifestError::Empty(field))
    } else if value.len() > MAX_STRING_BYTES {
        Err(ManifestError::TooLong(field))
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EditorMessage {
    Open {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        block_type: [u8; 16],
        account_id: [u8; 16],
        workspace_id: [u8; 16],
        editable: bool,
        metrics: ViewportMetrics,
    },
    Resize {
        instance: EditorInstanceId,
        metrics: ViewportMetrics,
    },
    Input {
        instance: EditorInstanceId,
        batch: InputBatch,
    },
    EditabilityChanged {
        instance: EditorInstanceId,
        editable: bool,
    },
    Close {
        instance: EditorInstanceId,
    },
    Surface {
        instance: EditorInstanceId,
        descriptor: SurfaceDescriptor,
    },
    Frame {
        instance: EditorInstanceId,
        frame: FrameReady,
    },
    Acknowledged {
        instance: EditorInstanceId,
        request_id: u64,
    },
    Failure {
        instance: EditorInstanceId,
        request_id: Option<u64>,
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelegatedClientMessage {
    Watch {
        instance: EditorInstanceId,
        request_id: u64,
        block_id: [u8; 16],
        block_type: [u8; 16],
    },
    Unwatch {
        instance: EditorInstanceId,
        request_id: u64,
        block_id: [u8; 16],
    },
    Snapshot {
        instance: EditorInstanceId,
        request_id: u64,
        block_id: [u8; 16],
        author: [u8; 16],
        sequence: u64,
        access: u8,
        data: Vec<u8>,
    },
    Operate {
        instance: EditorInstanceId,
        request_id: u64,
        block_id: [u8; 16],
        operation_id: [u8; 16],
        sequence: u64,
        operation: Vec<u8>,
    },
    Acknowledge {
        instance: EditorInstanceId,
        request_id: u64,
        block_id: [u8; 16],
        operation_id: [u8; 16],
        sequence: u64,
    },
    RemoteOperation {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        operation_id: [u8; 16],
        sequence: u64,
        operation: Vec<u8>,
    },
    AccessChanged {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        access: u8,
    },
    Error {
        instance: EditorInstanceId,
        request_id: Option<u64>,
        message: String,
    },
    Disconnected {
        instance: EditorInstanceId,
        message: String,
    },
}

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
    Editor(EditorMessage),
    Client(DelegatedClientMessage),
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
        Message::Editor(value) => validate_editor(value),
        Message::Client(value) => validate_client(value),
        _ => Ok(()),
    }
}

fn validate_editor(message: &EditorMessage) -> Result<(), DecodeError> {
    match message {
        EditorMessage::Input { batch, .. } => {
            collection(batch.events.len())?;
            for event in &batch.events {
                if let InputEvent::Key { logical, .. } | InputEvent::Text(logical) = event {
                    string(logical)?;
                }
            }
            Ok(())
        }
        EditorMessage::Surface { descriptor, .. } => {
            if descriptor.opaque.len() > MAX_OPAQUE_DESCRIPTOR_BYTES {
                return Err(DecodeError::LimitExceeded("surface descriptor"));
            }
            collection(descriptor.attachments.len())
        }
        EditorMessage::Frame { frame, .. } => {
            collection(frame.damage.len())?;
            collection(frame.attachments.len())
        }
        EditorMessage::Failure { message, .. } => string(message),
        _ => Ok(()),
    }
}

fn validate_client(message: &DelegatedClientMessage) -> Result<(), DecodeError> {
    match message {
        DelegatedClientMessage::Snapshot { data, .. }
        | DelegatedClientMessage::Operate {
            operation: data, ..
        }
        | DelegatedClientMessage::RemoteOperation {
            operation: data, ..
        } => {
            if data.len() > MAX_BLOCK_PAYLOAD_BYTES {
                Err(DecodeError::LimitExceeded("block payload"))
            } else {
                Ok(())
            }
        }
        DelegatedClientMessage::Error { message, .. }
        | DelegatedClientMessage::Disconnected { message, .. } => string(message),
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
