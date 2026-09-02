use bincode::Options;
use serde::{Deserialize, Serialize};
use std::fmt;

mod manifest;
mod session;
pub use manifest::{manifest_from_json, ManifestDocument};
pub use session::{HostSession, QueueError, SessionFailure, SessionState};

pub const PROTOCOL_VERSION: u16 = 43;
pub const MAX_COLLECTION_ITEMS: usize = 1024;
pub const MAX_STRING_BYTES: usize = 16 * 1024;
pub const MAX_OPAQUE_DESCRIPTOR_BYTES: usize = 64 * 1024;
pub const MAX_QUEUED_MESSAGES: usize = 256;
pub const MAX_CHILDREN: usize = 256;
pub const MAX_SURFACE_SIDE: u32 = 8192;
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
    pub frame: Option<FrameSpec>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameChrome {
    Drawn,
    Reserved,
    #[default]
    None,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FrameSpec {
    pub chrome: FrameChrome,
    pub content: Option<ChildRect>,
    pub trail: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameReport {
    pub screen: ScreenId,
    pub content: ChildRect,
    pub painted: Vec<ChildRect>,
    pub floating: Vec<ChildRect>,
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
    pub fn packed(screens: &[ScreenRequest]) -> Self {
        let mut slots: Vec<&ScreenRequest> = screens
            .iter()
            .filter(|request| request.metrics.pixel_width > 0 && request.metrics.pixel_height > 0)
            .collect();
        slots.sort_by_key(|request| {
            (
                std::cmp::Reverse(request.metrics.pixel_height),
                request.screen.0,
            )
        });
        let widest = slots
            .iter()
            .map(|request| request.metrics.pixel_width)
            .max()
            .unwrap_or(0);
        let area: u64 = slots
            .iter()
            .map(|request| {
                u64::from(request.metrics.pixel_width) * u64::from(request.metrics.pixel_height)
            })
            .sum();
        let shelf_width = widest
            .max((area as f64).sqrt().ceil() as u32)
            .min(MAX_SURFACE_SIDE)
            .max(widest);
        let mut layout = Self::default();
        let mut x = 0;
        let mut shelf_top = 0;
        let mut shelf_height = 0;
        for request in slots {
            let metrics = &request.metrics;
            if x > 0 && x + metrics.pixel_width > shelf_width {
                shelf_top += shelf_height;
                shelf_height = 0;
                x = 0;
            }
            layout.screens.push(ScreenPlacement {
                screen: request.screen,
                instance: request.instance,
                region: request.region,
                x,
                y: shelf_top,
                width: metrics.pixel_width,
                height: metrics.pixel_height,
                scale_factor_millis: (metrics.scale_factor * 1000.0).round().max(1.0) as u32,
            });
            x += metrics.pixel_width;
            shelf_height = shelf_height.max(metrics.pixel_height);
            layout.width = layout.width.max(x);
            layout.height = layout.height.max(shelf_top + shelf_height);
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
    Live,
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
    pub interaction: InteractionMode,
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
    pub chrome: Vec<EditorBand>,
    pub entry_point: String,
    pub network: Vec<String>,
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
    Frame,
    Preview,
    ArtifactSettings,
}

impl EditorRegion {
    pub const ALL: [Self; 3] = [Self::Frame, Self::Preview, Self::ArtifactSettings];
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorBand {
    Toolbar,
    LeftSidebar,
    RightSidebar,
}

impl EditorBand {
    pub const ALL: [Self; 3] = [Self::Toolbar, Self::LeftSidebar, Self::RightSidebar];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    Malformed(String),
    Empty(&'static str),
    TooLong(&'static str),
    InvalidIdentity,
    InvalidBlockType,
    InvalidRegions,
    InvalidChrome,
    InvalidNetworkHost,
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
                formatter.write_str("the regions must include the frame exactly once")
            }
            Self::InvalidChrome => formatter.write_str("a chrome band is declared twice"),
            Self::InvalidNetworkHost => {
                formatter.write_str("a network host is not a plain host name")
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
        if !self.regions.contains(&EditorRegion::Frame)
            || EditorRegion::ALL
                .iter()
                .any(|region| self.regions.iter().filter(|it| *it == region).count() > 1)
        {
            return Err(ManifestError::InvalidRegions);
        }
        if EditorBand::ALL
            .iter()
            .any(|band| self.chrome.iter().filter(|it| *it == band).count() > 1)
        {
            return Err(ManifestError::InvalidChrome);
        }
        manifest_string("entry point", &self.entry_point)?;
        for host in &self.network {
            manifest_string("network host", host)?;
            if !host
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
            {
                return Err(ManifestError::InvalidNetworkHost);
            }
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
        client_id: [u8; 16],
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
    Present {
        instance: EditorInstanceId,
        presenting: bool,
    },
    PresentingChanged {
        instance: EditorInstanceId,
        presenting: bool,
    },
    Resized {
        instance: EditorInstanceId,
        width: f32,
        height: f32,
    },
    LeaveFrame {
        instance: EditorInstanceId,
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

    FileDrop {
        instance: EditorInstanceId,
        region: EditorRegion,
        x: f32,
        y: f32,
        files: Vec<DroppedFile>,
        dropped: bool,
    },

    FileDropLeft {
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
    PasteImage {
        instance: EditorInstanceId,
        request_id: u64,
    },
    ImagePasted {
        instance: EditorInstanceId,
        request_id: u64,
        image: ClipboardImage,
    },
    PlayAudio {
        instance: EditorInstanceId,
        block_id: [u8; 16],
        command: AudioCommand,
    },
    AudioStatus {
        instance: EditorInstanceId,
        status: AudioStatus,
    },
    Fetch {
        instance: EditorInstanceId,
        request_id: u64,
        url: String,
    },
    Fetched {
        instance: EditorInstanceId,
        request_id: u64,
        result: FetchResult,
    },
    GrabCursor {
        instance: EditorInstanceId,
        grabbed: bool,
    },
    WebView {
        instance: EditorInstanceId,
        region: EditorRegion,
        rect: Option<ChildRect>,
    },
    WebViewCommand {
        instance: EditorInstanceId,
        command: WebViewCommand,
    },
    WebViewEvent {
        instance: EditorInstanceId,
        event: WebViewEvent,
    },
    ReadAsset {
        instance: EditorInstanceId,
        request_id: u64,
        name: String,
    },
    AssetRead {
        instance: EditorInstanceId,
        request_id: u64,
        result: AssetResult,
    },
    OpenCreation {
        instance: EditorInstanceId,
        account_id: [u8; 16],
        workspace_id: [u8; 16],
        client_id: [u8; 16],
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
        client_id: [u8; 16],
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
    Ime {
        instance: EditorInstanceId,
        region: EditorRegion,
        area: Option<ImeArea>,
    },
    Presence {
        instance: EditorInstanceId,
        visible: bool,
        entries: Vec<PresenceEntry>,
    },
    PublishPresence {
        instance: EditorInstanceId,
        presence_id: [u8; 16],
        data: Option<Vec<u8>>,
    },
    RevealPresence {
        instance: EditorInstanceId,
        client_id: u64,
    },
    ReplaceChild {
        instance: EditorInstanceId,
        request_id: u64,
        old: [u8; 16],
        new: [u8; 16],
    },
    ChildReplaced {
        instance: EditorInstanceId,
        request_id: u64,
        replaced: bool,
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
    pub templates: bool,
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
pub enum FetchResult {
    Body(Vec<u8>),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetResult {
    Body(Vec<u8>),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebViewCommand {
    Open(String),
    Load(String),
    Reload,
    FocusApp,
    Close,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebViewEvent {
    Navigate(String),
    Finished(String),
    Push(String),
    Replace(String),
    Title(String),
    History(i32),
    NewWindow(String),
    Address(String),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DroppedFile {
    pub name: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardImage {
    Pasted { name: String, data: Vec<u8> },
    Empty,
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCommand {
    Toggle,
    Reset,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioStatus {
    pub playing: bool,
    pub position_micros: u64,
    pub duration_micros: Option<u64>,
    pub error: Option<String>,
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
    Frames(Vec<FrameReport>),
    Input(InputBatch),
    DrawFrame,
    FrameNeeded,
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
    Surface,
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
    PointerMotion {
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
    Paste(String),
    Ime(ImeInput),
    Modifiers(Modifiers),
    Focus(bool),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImeInput {
    Enabled,
    Preedit(String),
    Commit(String),
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresenceEntry {
    pub client_id: u64,
    pub presence_id: [u8; 16],
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImeArea {
    pub rect: ChildRect,
    pub cursor: ChildRect,
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
pub struct FrameReady {
    pub generation: u64,
    pub repaint_after_micros: Option<u64>,
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
                if let InputEvent::Key { logical, .. }
                | InputEvent::Text(logical)
                | InputEvent::Paste(logical)
                | InputEvent::Ime(ImeInput::Preedit(logical) | ImeInput::Commit(logical)) = event
                {
                    string(logical)?;
                }
            }
            Ok(())
        }
        Message::Screens(value) => {
            collection(value.screens.len())?;
            for request in &value.screens {
                if let Some(frame) = &request.frame {
                    collection(frame.trail.len())?;
                    strings(&frame.trail)?;
                }
            }
            Ok(())
        }
        Message::Frames(value) => {
            collection(value.len())?;
            for report in value {
                collection(report.painted.len())?;
                collection(report.floating.len())?;
            }
            Ok(())
        }
        Message::Layout(value) => collection(value.screens.len()),
        Message::RegionSizes(value) => collection(value.len()),
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
        EditorMessage::FileDrop { files, .. } => {
            collection(files.len())?;
            strings(files.iter().map(|file| &file.name))
        }
        EditorMessage::ImagePasted { image, .. } => match image {
            ClipboardImage::Pasted { name, .. } => string(name),
            ClipboardImage::Failed(message) => string(message),
            ClipboardImage::Empty => Ok(()),
        },
        EditorMessage::AudioStatus { status, .. } => match &status.error {
            Some(message) => string(message),
            None => Ok(()),
        },
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
        EditorMessage::Presence { entries, .. } => collection(entries.len()),
        EditorMessage::Fetch { url, .. } => string(url),
        EditorMessage::Fetched { result, .. } => match result {
            FetchResult::Failed(message) => string(message),
            FetchResult::Body(_) => Ok(()),
        },
        EditorMessage::WebViewCommand { command, .. } => match command {
            WebViewCommand::Open(url) | WebViewCommand::Load(url) => string(url),
            WebViewCommand::Reload | WebViewCommand::FocusApp | WebViewCommand::Close => Ok(()),
        },
        EditorMessage::WebViewEvent { event, .. } => match event {
            WebViewEvent::Navigate(value)
            | WebViewEvent::Finished(value)
            | WebViewEvent::Push(value)
            | WebViewEvent::Replace(value)
            | WebViewEvent::Title(value)
            | WebViewEvent::NewWindow(value)
            | WebViewEvent::Address(value)
            | WebViewEvent::Failed(value) => string(value),
            WebViewEvent::History(_) => Ok(()),
        },
        EditorMessage::ReadAsset { name, .. } => string(name),
        EditorMessage::AssetRead { result, .. } => match result {
            AssetResult::Failed(message) => string(message),
            AssetResult::Body(_) => Ok(()),
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
