use bincode::Options;
use serde::{Deserialize, Serialize};
use std::fmt;

mod attachment;
#[cfg(any(unix, windows))]
pub mod desktop_attachments;
mod linux_surface;
mod manifest;
mod session;
mod windows_surface;
pub use attachment::{
    validate_attachments, AttachmentDescriptor, AttachmentError, AttachmentOwnership,
    AttachmentType, MAX_ATTACHMENTS,
};
pub use linux_surface::{
    LinuxGraphicsBackend, LinuxSurfaceDescriptor, LinuxSurfaceError, LinuxSurfaceLifecycle,
    LinuxSurfacePlane, LinuxSurfaceState,
};
pub use manifest::{manifest_from_json, ManifestDocument};
pub use session::{HostSession, QueueError, SessionFailure, SessionState};
pub use windows_surface::{
    WindowsSurfaceDescriptor, WindowsSurfaceError, WindowsSurfaceLifecycle, WindowsSurfaceState,
};

pub const PROTOCOL_VERSION: u16 = 25;
pub const MAX_COLLECTION_ITEMS: usize = 1024;
pub const MAX_STRING_BYTES: usize = 16 * 1024;
pub const MAX_OPAQUE_DESCRIPTOR_BYTES: usize = 64 * 1024;
pub const MAX_QUEUED_MESSAGES: usize = 256;
pub const MAX_CHILDREN: usize = 256;
pub const REQUEST_TIMEOUT_MILLISECONDS: u64 = 5_000;

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

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildLayer {
    #[default]
    Below,
    Above,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildMode {
    Preview,
    #[default]
    Passive,
    Active,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChildRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChildPlacement {
    pub child: ChildId,
    pub block_id: [u8; 16],
    pub block_type: [u8; 16],
    pub rect: ChildRect,
    pub clip: ChildRect,
    pub corner_radius: f32,
    pub layer: ChildLayer,
    pub mode: ChildMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Occluder {
    pub after: u32,
    pub rect: ChildRect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChildPlacements {
    pub instance: EditorInstanceId,
    pub region: EditorRegion,
    pub generation: u64,
    pub children: Vec<ChildPlacement>,
    pub occluders: Vec<Occluder>,
}

impl ChildPlacements {
    pub fn occluded(&self, index: usize, x: f32, y: f32) -> bool {
        self.occluders
            .iter()
            .filter(|occluder| occluder.after as usize > index)
            .any(|occluder| occluder.rect.contains(x, y))
    }
}

impl ChildRect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChildStatus {
    pub instance: EditorInstanceId,
    pub region: EditorRegion,
    pub child: ChildId,
    pub available: bool,
    pub intrinsic_width: f32,
    pub intrinsic_height: f32,
    pub aspect_ratio: f32,
    pub hovered: bool,
    pub active: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub identity: PluginIdentity,
    pub block_type: [u8; 16],
    pub display_name: String,
    pub icon: String,
    pub creation: CreationMode,
    pub children: ChildOperations,
    pub important: bool,
    pub interaction: InteractionMode,
    pub capabilities: EditorCapabilities,
    pub resize: ResizeMode,
    pub regions: Vec<EditorRegion>,
    pub entry_points: EntryPoints,
    pub surfaces: Vec<SurfaceMechanism>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildOperations {
    pub add: bool,
    pub delete: bool,
    pub replace: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum InteractionMode {
    Preview,
    #[default]
    Live,
    Playback,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorCapabilities {
    pub rotation: bool,
    pub preserve_aspect_ratio: bool,
    pub pan_and_zoom: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResizeMode {
    None,
    Horizontal,
    Vertical,
    #[default]
    Both,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTypeDescriptor {
    pub block_type: [u8; 16],
    pub display_name: String,
    pub icon_codepoint: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreationMode {
    Immediate,
    Dialog,

    None,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorRegion {
    Main,
    Toolbar,
    LeftSidebar,
    RightSidebar,
    Preview,
    ArtifactSettings,
}

impl EditorRegion {
    pub const ALL: [Self; 6] = [
        Self::Main,
        Self::Toolbar,
        Self::LeftSidebar,
        Self::RightSidebar,
        Self::Preview,
        Self::ArtifactSettings,
    ];
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryPoints {
    #[serde(default)]
    pub web: Option<String>,
    #[serde(default)]
    pub windows: Option<String>,
    #[serde(default)]
    pub linux: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    Malformed(String),
    Empty(&'static str),
    TooLong(&'static str),
    InvalidIdentity,
    InvalidBlockType,
    InvalidRegions,
    MissingEntryPoint,
    MissingSurface,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(error) => write!(formatter, "the manifest could not be read: {error}"),
            Self::Empty(field) => write!(formatter, "{field} is empty"),
            Self::TooLong(field) => write!(formatter, "{field} is too long"),
            Self::InvalidIdentity => formatter.write_str("the plugin id is not a plain identifier"),
            Self::InvalidBlockType => formatter.write_str("the block type is not a uuid"),
            Self::InvalidRegions => {
                formatter.write_str("the regions must include the main one exactly once")
            }
            Self::MissingEntryPoint => formatter.write_str("no entry point is given"),
            Self::MissingSurface => {
                formatter.write_str("an entry point has no surface mechanism to present with")
            }
        }
    }
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
        let entries = [
            &self.entry_points.web,
            &self.entry_points.windows,
            &self.entry_points.linux,
        ];
        if entries.iter().all(|entry| entry.is_none()) {
            return Err(ManifestError::MissingEntryPoint);
        }
        for entry in entries.into_iter().flatten() {
            manifest_string("entry point", entry)?;
        }
        let web_valid = self.entry_points.web.is_none()
            || self.surfaces.contains(&SurfaceMechanism::WebExternalImage);
        let windows_valid = self.entry_points.windows.is_none()
            || self.surfaces.contains(&SurfaceMechanism::WindowsDxgi);
        let linux_valid = self.entry_points.linux.is_none()
            || self.surfaces.contains(&SurfaceMechanism::LinuxDmaBuf);
        if !web_valid || !windows_valid || !linux_valid {
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
    ViewChanged {
        instance: EditorInstanceId,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    ChangeView {
        instance: EditorInstanceId,
        change: ViewChange,
    },
    Close {
        instance: EditorInstanceId,
    },

    OpenBlock {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        block_type: [u8; 16],
    },

    DragOver {
        instance: EditorInstanceId,
        region: EditorRegion,
        x: f32,
        y: f32,
        block_id: [u8; 16],
        block_type: [u8; 16],
        dropped: bool,
    },

    DragLeft {
        instance: EditorInstanceId,
    },

    DragAccepted {
        instance: EditorInstanceId,
        accepted: bool,
    },
    PickFile {
        instance: EditorInstanceId,
        request_id: u64,
        filter: FileFilter,
    },
    FilePicked {
        instance: EditorInstanceId,
        request_id: u64,
        pick: FilePick,
    },
    PickBlock {
        instance: EditorInstanceId,
        request_id: u64,
        filter: BlockFilter,
    },
    BlockPicked {
        instance: EditorInstanceId,
        request_id: u64,
        pick: BlockPick,
    },
    OpenCreation {
        instance: EditorInstanceId,
        account_id: [u8; 16],
        workspace_id: [u8; 16],
    },
    CreationReady {
        instance: EditorInstanceId,
        ready: bool,
    },
    CommitCreation {
        instance: EditorInstanceId,
    },
    CreationBlock {
        instance: EditorInstanceId,
        outcome: CreationOutcome,
    },
    OpenArtifact {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        block_type: [u8; 16],
        account_id: [u8; 16],
        workspace_id: [u8; 16],
        data: Vec<u8>,
    },
    ArtifactSettings {
        instance: EditorInstanceId,
        data: Vec<u8>,
    },
    ArtifactDescribed {
        instance: EditorInstanceId,
        description: ArtifactDescription,
    },
    ArtifactEdited {
        instance: EditorInstanceId,
        data: Vec<u8>,
    },
    RegenerateArtifact {
        instance: EditorInstanceId,
        data: Vec<u8>,
    },
    ArtifactRegenerated {
        instance: EditorInstanceId,
        outcome: RegenerationOutcome,
    },
    Cursor {
        instance: EditorInstanceId,
        region: EditorRegion,
        cursor: CursorIcon,
    },
    AspectRatio {
        instance: EditorInstanceId,
        ratio: f32,
    },

    IntrinsicSize {
        instance: EditorInstanceId,
        width: f32,
        height: f32,
    },
    Performance {
        instance: EditorInstanceId,
        group: String,
        measurements: Vec<PerformanceMeasurement>,
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
pub enum PerformanceMeasurement {
    Duration { name: String, nanoseconds: u64 },
    Count { name: String, count: u64 },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFilter {
    pub name: String,
    pub default_file_name: String,
    pub extensions: Vec<String>,
    pub mime_types: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorIcon {
    #[default]
    Default,
    None,
    Pointer,
    Text,
    Crosshair,
    Grab,
    Grabbing,
    Move,
    NotAllowed,
    Wait,
    Progress,
    Help,
    ResizeHorizontal,
    ResizeVertical,
    ResizeNeSw,
    ResizeNwSe,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactDescription {
    Described { source: [u8; 16], summary: String },
    Unreadable(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegenerationOutcome {
    Done,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreationOutcome {
    Created([u8; 16]),
    Failed(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockFilter {
    pub name: String,
    pub block_types: Vec<[u8; 16]>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockPick {
    Chosen {
        block_id: [u8; 16],
        block_type: [u8; 16],
    },
    Cancelled,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilePick {
    Chosen { name: String, data: Vec<u8> },
    Cancelled,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunnelMessage {
    Request { payload: String },

    Response { payload: String },
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
    Surface(SurfaceDescriptor),
    DrawFrame,
    FrameReady(FrameReady),
    Acknowledged { request_id: u64 },
    Ping { nonce: u64 },
    Pong { nonce: u64 },
    Error(ProtocolError),
    Shutdown,
    ShutdownAcknowledged,
    Editor(EditorMessage),
    Client(TunnelMessage),
    BlockTypes(Vec<BlockTypeDescriptor>),
    Children(ChildPlacements),
    ChildStatuses(Vec<ChildStatus>),
}

impl Message {
    pub fn is_session(&self) -> bool {
        matches!(
            self,
            Self::Hello(_)
                | Self::HelloAccepted(_)
                | Self::HelloRejected(_)
                | Self::Acknowledged { .. }
                | Self::Ping { .. }
                | Self::Pong { .. }
                | Self::Error(_)
                | Self::Shutdown
                | Self::ShutdownAcknowledged
        )
    }
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
    pub dark_theme: bool,
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
    WindowsDxgi,
    LinuxDmaBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ViewChange {
    Pan {
        x: f32,
        y: f32,
    },
    Zoom {
        factor: f32,
        anchor: Option<(f32, f32)>,
    },
    Fit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewportMetrics {
    pub logical_width: f32,
    pub logical_height: f32,
    pub visible_x: f32,
    pub visible_y: f32,
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
    Zoom {
        factor: f32,
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
    pub repaint_after_micros: Option<u64>,
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
        Message::BlockTypes(value) => {
            collection(value.len())?;
            for descriptor in value {
                string(&descriptor.display_name)?;
                string(&descriptor.icon_codepoint)?;
            }
            Ok(())
        }
        Message::Children(value) => validate_children(value),
        Message::ChildStatuses(value) => {
            collection(value.len())?;
            for status in value {
                if let Some(error) = &status.error {
                    string(error)?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_children(placements: &ChildPlacements) -> Result<(), DecodeError> {
    if placements.children.len() > MAX_CHILDREN {
        return Err(DecodeError::LimitExceeded("children"));
    }
    if placements.occluders.len() > MAX_COLLECTION_ITEMS {
        return Err(DecodeError::LimitExceeded("occluders"));
    }
    let mut covered = 0;
    for occluder in &placements.occluders {
        if occluder.after as usize > placements.children.len()
            || (occluder.after as usize) < covered
        {
            return Err(DecodeError::MalformedPayload);
        }
        covered = occluder.after as usize;
    }
    Ok(())
}

fn validate_editor(message: &EditorMessage) -> Result<(), DecodeError> {
    match message {
        EditorMessage::Failure { message, .. } => string(message),
        EditorMessage::PickFile { filter, .. } => {
            string(&filter.name)?;
            string(&filter.default_file_name)?;
            collection(filter.extensions.len())?;
            collection(filter.mime_types.len())?;
            strings(filter.extensions.iter().chain(&filter.mime_types))
        }
        EditorMessage::Performance {
            group,
            measurements,
            ..
        } => {
            string(group)?;
            collection(measurements.len())?;
            strings(measurements.iter().map(|measurement| match measurement {
                PerformanceMeasurement::Duration { name, .. }
                | PerformanceMeasurement::Count { name, .. } => name,
            }))
        }
        EditorMessage::CreationBlock { outcome, .. } => match outcome {
            CreationOutcome::Created(_) => Ok(()),
            CreationOutcome::Failed(message) => string(message),
        },
        EditorMessage::ArtifactDescribed { description, .. } => match description {
            ArtifactDescription::Described { summary, .. } => string(summary),
            ArtifactDescription::Unreadable(message) => string(message),
        },
        EditorMessage::ArtifactRegenerated { outcome, .. } => match outcome {
            RegenerationOutcome::Done => Ok(()),
            RegenerationOutcome::Failed(message) => string(message),
        },
        EditorMessage::OpenArtifact { data, .. }
        | EditorMessage::ArtifactSettings { data, .. }
        | EditorMessage::ArtifactEdited { data, .. }
        | EditorMessage::RegenerateArtifact { data, .. } => descriptor(data),
        EditorMessage::FilePicked { pick, .. } => match pick {
            FilePick::Chosen { name, .. } => string(name),
            FilePick::Failed(message) => string(message),
            FilePick::Cancelled => Ok(()),
        },
        EditorMessage::PickBlock { filter, .. } => {
            string(&filter.name)?;
            collection(filter.block_types.len())
        }
        EditorMessage::BlockPicked { pick, .. } => match pick {
            BlockPick::Failed(message) => string(message),
            BlockPick::Chosen { .. } | BlockPick::Cancelled => Ok(()),
        },
        _ => Ok(()),
    }
}

fn descriptor(data: &[u8]) -> Result<(), DecodeError> {
    if data.len() > MAX_OPAQUE_DESCRIPTOR_BYTES {
        Err(DecodeError::LimitExceeded("artifact settings"))
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
        .reject_trailing_bytes()
}

#[cfg(test)]
mod tests;
