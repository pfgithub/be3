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

pub const PROTOCOL_VERSION: u16 = 6;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_COLLECTION_ITEMS: usize = 1024;
pub const MAX_STRING_BYTES: usize = 16 * 1024;
pub const MAX_OPAQUE_DESCRIPTOR_BYTES: usize = 64 * 1024;
pub const MAX_QUEUED_MESSAGES: usize = 256;
pub const REQUEST_TIMEOUT_MILLISECONDS: u64 = 5_000;
pub const MAX_BLOCK_PAYLOAD_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorInstanceId(pub u64);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenRequest {
    pub screen: ScreenId,
    pub instance: EditorInstanceId,
    pub region: EditorRegion,
    pub metrics: ViewportMetrics,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenSet {
    pub request_id: u64,
    pub screens: Vec<ScreenRequest>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenPlacement {
    pub screen: ScreenId,
    pub instance: EditorInstanceId,
    pub region: EditorRegion,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub scale_factor_millis: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegionSize {
    pub screen: ScreenId,
    pub logical_width: f32,
    pub logical_height: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenLayout {
    pub generation: u64,
    pub width: u32,
    pub height: u32,
    pub screens: Vec<ScreenPlacement>,
}

impl ScreenPlacement {
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor_millis as f32 / 1000.0
    }
}

impl ScreenLayout {
    pub fn stacked(screens: &[ScreenRequest]) -> Self {
        let mut layout = Self::default();
        for request in screens {
            let metrics = &request.metrics;
            if metrics.pixel_width == 0 || metrics.pixel_height == 0 {
                continue;
            }
            layout.screens.push(ScreenPlacement {
                screen: request.screen,
                instance: request.instance,
                region: request.region,
                x: 0,
                y: layout.height,
                width: metrics.pixel_width,
                height: metrics.pixel_height,
                scale_factor_millis: (metrics.scale_factor * 1000.0).round().max(1.0) as u32,
            });
            layout.width = layout.width.max(metrics.pixel_width);
            layout.height += metrics.pixel_height;
        }
        layout
    }

    pub fn placement(&self, screen: ScreenId) -> Option<&ScreenPlacement> {
        self.screens
            .iter()
            .find(|placement| placement.screen == screen)
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn same_placements(&self, other: &Self) -> bool {
        self.width == other.width && self.height == other.height && self.screens == other.screens
    }
}

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

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorRegion {
    Main,
    Toolbar,
    LeftSidebar,
    RightSidebar,
}

impl EditorRegion {
    pub const ALL: [Self; 4] = [
        Self::Main,
        Self::Toolbar,
        Self::LeftSidebar,
        Self::RightSidebar,
    ];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryPoints {
    pub web: Option<String>,
    pub windows: Option<String>,
    pub android: Option<String>,
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
        if !self.regions.contains(&EditorRegion::Main)
            || EditorRegion::ALL
                .iter()
                .any(|region| self.regions.iter().filter(|it| *it == region).count() > 1)
        {
            return Err(ManifestError::InvalidRegions);
        }
        if self.entry_points.web.is_none()
            && self.entry_points.windows.is_none()
            && self.entry_points.android.is_none()
        {
            return Err(ManifestError::MissingEntryPoint);
        }
        for entry in [
            &self.entry_points.web,
            &self.entry_points.windows,
            &self.entry_points.android,
        ]
        .into_iter()
        .flatten()
        {
            manifest_string("entry point", entry)?;
        }
        let web_valid = self.entry_points.web.is_none()
            || self.surfaces.contains(&SurfaceMechanism::WebExternalImage);
        let windows_valid = self.entry_points.windows.is_none()
            || self.surfaces.contains(&SurfaceMechanism::WindowsDxgi);
        let android_valid = self.entry_points.android.is_none()
            || self
                .surfaces
                .contains(&SurfaceMechanism::AndroidHardwareBuffer);
        if !web_valid || !windows_valid || !android_valid {
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
    },
    EditabilityChanged {
        instance: EditorInstanceId,
        editable: bool,
    },
    Close {
        instance: EditorInstanceId,
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

/// A block-client frame tunnelled between an editor instance's client and the
/// host's connection. The payloads are the ordinary JSON of the block
/// protocol, so the host forwards them without needing to understand them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunnelMessage {
    /// A client message on its way to the server.
    Request {
        instance: EditorInstanceId,
        payload: String,
    },
    /// A server message on its way back to the instance's client.
    Response {
        instance: EditorInstanceId,
        payload: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Message {
    Hello(Hello),
    HelloAccepted(HelloAccepted),
    HelloRejected(ProtocolError),
    Screens(ScreenSet),
    Layout(ScreenLayout),
    RegionSizes(Vec<RegionSize>),
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
    Client(TunnelMessage),
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
pub struct ViewportMetrics {
    pub logical_width: f32,
    pub logical_height: f32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub scale_factor: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputBatch {
    pub screen: ScreenId,
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
        Message::Screens(value) => collection(value.screens.len()),
        Message::Layout(value) => collection(value.screens.len()),
        Message::RegionSizes(value) => collection(value.len()),
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
        EditorMessage::Failure { message, .. } => string(message),
        _ => Ok(()),
    }
}

fn validate_client(message: &TunnelMessage) -> Result<(), DecodeError> {
    let (TunnelMessage::Request { payload, .. } | TunnelMessage::Response { payload, .. }) =
        message;
    if payload.len() > MAX_BLOCK_PAYLOAD_BYTES {
        Err(DecodeError::LimitExceeded("block payload"))
    } else {
        Ok(())
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
