use block_client::{blocks::audio::Audio, BlockClient, Tunnel};
use block_plugin_api::{
    ArtifactDescription, AssetResult, AudioCommand, AudioStatus, BlockPick, BlockTypeDescriptor,
    ChildId, ChildMode, ChildPlacement, ChildPlacements, ChildStatus, ClipboardImage,
    CreationOutcome, CursorIcon, EditorInstanceId, EditorMessage, EditorRegion, FetchResult,
    FilePick, FrameReport, FrameSpec, ImeArea, Message, Occluder, PerformanceMeasurement,
    PresenceEntry, RegenerationOutcome, RegionSize, ScreenId, ScreenLayout, ScreenRequest,
    ScreenSet, TunnelMessage, ViewChange,
};
use eframe::egui;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;

use super::{
    audio::AudioPlayer,
    input::{viewport_metrics, BlockDragEvent, FileDropEvent, InputAdapter},
    pieces, BlockPickRequest, EditorBlock, HostChild, HostChildStatus, InstanceRole,
    PresencePublication, MAX_LIVE_CHILDREN,
};
use crate::{
    performance,
    platform::{assets::Asset, http::Fetch, FileFilter, FilePicker},
    plugin_host::web_view::WebViewHost,
};

const FETCH_POLL_INTERVAL: Duration = Duration::from_millis(100);
const REFUSED: &str = "this plugin's manifest does not allow it to reach";

#[derive(Default)]
pub(super) struct Instances {
    entries: HashMap<EditorInstanceId, Instance>,
    connection: Option<Connection>,
    next_screen: u64,
    announced: HashSet<ScreenId>,
    request_id: u64,
    block_types: Option<Arc<Vec<BlockTypeDescriptor>>>,
    sent_block_types: bool,
    network: Vec<String>,
}

struct Connection {
    client: Arc<BlockClient>,
    client_id: Uuid,
    tunnel: Tunnel,
}

struct Instance {
    context: egui::Context,
    role: InstanceRole,
    artifact: ArtifactState,
    creation_ready: bool,
    created: Option<Result<Uuid, String>>,
    screens: HashMap<EditorRegion, Screen>,
    opened: bool,
    opens: Vec<(Uuid, Uuid)>,
    drag_accepted: bool,
    intrinsic: Option<egui::Vec2>,
    aspect_ratio: Option<f32>,
    picks: Vec<PendingPick>,
    fetches: Vec<PendingFetch>,
    assets: Vec<PendingAsset>,
    pastes: Vec<PendingPaste>,
    audio: Option<AudioPlayer>,
    reported_audio: AudioStatus,
    reported_size: Option<egui::Vec2>,
    block_picks: Vec<BlockPickRequest>,
    view: Option<EditorView>,
    reported_view: Option<EditorView>,
    view_changes: Vec<ViewChange>,
    presenting: bool,
    reported_presenting: bool,
    grabbed: bool,
    web_view: Option<WebViewHost>,
    web_view_rect: Option<(EditorRegion, block_plugin_api::ChildRect)>,
    presence: Option<(bool, Vec<PresenceEntry>)>,
    presence_publications: Vec<PresencePublication>,
    replacements: HashMap<(Uuid, Uuid), Replacement>,
    next_replacement: u64,
    leaving: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct EditorView {
    pub(crate) rect: egui::Rect,
    pub(crate) scale: f32,
}

enum Replacement {
    Pending(u64),
    Answered(bool),
}

#[derive(Default)]
struct ArtifactState {
    data: Vec<u8>,
    description: Option<ArtifactDescription>,
    draft: Option<Vec<u8>>,
    outcome: Option<Result<(), String>>,
}

impl Instance {
    fn new(context: &egui::Context, role: InstanceRole) -> Self {
        Self {
            context: context.clone(),
            role,
            artifact: ArtifactState::default(),
            creation_ready: false,
            created: None,
            screens: HashMap::new(),
            opened: false,
            opens: Vec::new(),
            drag_accepted: false,
            intrinsic: None,
            aspect_ratio: None,
            picks: Vec::new(),
            fetches: Vec::new(),
            assets: Vec::new(),
            pastes: Vec::new(),
            audio: None,
            reported_audio: AudioStatus::default(),
            reported_size: None,
            block_picks: Vec::new(),
            view: None,
            reported_view: None,
            view_changes: Vec::new(),
            presenting: false,
            reported_presenting: false,
            grabbed: false,
            web_view: None,
            web_view_rect: None,
            presence: None,
            presence_publications: Vec::new(),
            replacements: HashMap::new(),
            next_replacement: 0,
            leaving: false,
        }
    }
}

struct PendingPick {
    request_id: u64,
    picker: FilePicker,
}

struct PendingFetch {
    request_id: u64,
    fetch: Fetch,
}

struct PendingAsset {
    request_id: u64,
    asset: Asset,
}

struct PendingPaste {
    request_id: u64,
    image: ClipboardImage,
}

#[derive(Clone, Copy)]
pub(super) struct Placement {
    pub(super) id: egui::Id,
    pub(super) rect: egui::Rect,
    pub(super) clip: egui::Rect,
    pub(super) pass: u64,
}

struct Screen {
    input: InputAdapter,
    placement: Option<Placement>,
    request: ScreenRequest,
    last_seen: u64,
    used: Option<egui::Vec2>,
    report: Option<FrameReport>,
    dragging: bool,
    file_dropping: bool,
    cursor: CursorIcon,
    ime: Option<ImeArea>,
    children: ChildTable,
    reported_statuses: HashMap<ChildId, ChildStatus>,
    revoked: HashSet<ChildId>,
    frame_revoked: HashSet<ChildId>,
}

#[derive(Default, PartialEq)]
struct ChildTable {
    generation: u64,
    size: egui::Vec2,
    children: Vec<ChildPlacement>,
    occluders: Vec<Occluder>,
}

struct Hole {
    rect: egui::Rect,
    occluders: Vec<egui::Rect>,
}

#[derive(Default)]
pub(super) struct Holes {
    holes: Vec<Hole>,
}

impl Holes {
    pub(super) fn contains(&self, position: egui::Pos2) -> bool {
        self.holes.iter().any(|hole| {
            hole.rect.contains(position)
                && !hole
                    .occluders
                    .iter()
                    .any(|occluder| occluder.contains(position))
        })
    }
}

pub(super) struct NextScreens {
    pub(super) opened: Vec<Message>,
    pub(super) screens: Vec<ScreenRequest>,
}

impl Instances {
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn remove(&mut self, instance: EditorInstanceId) -> bool {
        self.entries.remove(&instance).is_some()
    }

    fn connect(&mut self, context: &egui::Context, client: &Arc<BlockClient>, client_id: Uuid) {
        if self
            .connection
            .as_ref()
            .is_some_and(|connection| Arc::ptr_eq(&connection.client, client))
        {
            return;
        }
        let tunnel = client.open_tunnel({
            let context = context.clone();
            move || context.request_repaint()
        });
        self.connection = Some(Connection {
            client: Arc::clone(client),
            client_id,
            tunnel,
        });
    }

    pub(super) fn report(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        context: &egui::Context,
        client: &Arc<BlockClient>,
        client_id: Uuid,
        role: InstanceRole,
        block_types: &Arc<Vec<BlockTypeDescriptor>>,
        frame: Option<FrameSpec>,
        size: egui::Vec2,
        visible: egui::Rect,
        scale_factor: f32,
        pass: u64,
    ) -> ScreenId {
        self.connect(context, client, client_id);
        if self.block_types.is_none() {
            self.block_types = Some(Arc::clone(block_types));
        }
        let entry = self
            .entries
            .entry(instance)
            .or_insert_with(|| Instance::new(context, role));
        let next_screen = &mut self.next_screen;
        let screen = entry.screens.entry(region).or_insert_with(|| {
            *next_screen += 1;
            Screen {
                input: InputAdapter::default(),
                placement: None,
                request: ScreenRequest {
                    screen: ScreenId(*next_screen),
                    instance,
                    region,
                    metrics: viewport_metrics(size, visible, scale_factor),
                    frame: frame.clone(),
                },
                last_seen: pass,
                used: None,
                report: None,
                dragging: false,
                file_dropping: false,
                cursor: CursorIcon::Default,
                ime: None,
                children: ChildTable::default(),
                reported_statuses: HashMap::new(),
                revoked: HashSet::new(),
                frame_revoked: HashSet::new(),
            }
        });
        screen.request.metrics = viewport_metrics(size, visible, scale_factor);
        screen.request.frame = frame;
        screen.last_seen = pass;
        screen.request.screen
    }

    pub(super) fn set_view(&mut self, instance: EditorInstanceId, view: EditorView) {
        if let Some(entry) = self.entries.get_mut(&instance) {
            entry.view = Some(view);
        }
    }

    pub(super) fn presenting(&self, instance: EditorInstanceId) -> bool {
        self.entries
            .get(&instance)
            .is_some_and(|entry| entry.presenting)
    }

    pub(super) fn set_presenting(&mut self, instance: EditorInstanceId, presenting: bool) -> bool {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        let changed = entry.presenting != presenting;
        entry.presenting = presenting;
        changed
    }

    pub(super) fn resized(&mut self, instance: EditorInstanceId, size: egui::Vec2) -> Vec<Message> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        if entry.reported_size.is_some_and(|reported| {
            (reported.x - size.x).abs() < 0.01 && (reported.y - size.y).abs() < 0.01
        }) {
            return Vec::new();
        }
        entry.reported_size = Some(size);
        vec![Message::Editor(EditorMessage::Resized {
            instance,
            width: size.x,
            height: size.y,
        })]
    }

    pub(super) fn take_view_changes(&mut self, instance: EditorInstanceId) -> Vec<ViewChange> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.view_changes))
            .unwrap_or_default()
    }

    pub(super) fn report_creation(
        &mut self,
        instance: EditorInstanceId,
        context: &egui::Context,
        client: &Arc<BlockClient>,
        client_id: Uuid,
        block_types: &Arc<Vec<BlockTypeDescriptor>>,
    ) -> bool {
        self.connect(context, client, client_id);
        if self.block_types.is_none() {
            self.block_types = Some(Arc::clone(block_types));
        }
        self.entries
            .entry(instance)
            .or_insert_with(|| Instance::new(context, InstanceRole::Creation))
            .opened
    }

    pub(super) fn report_artifact(
        &mut self,
        instance: EditorInstanceId,
        context: &egui::Context,
        client: &Arc<BlockClient>,
        client_id: Uuid,
        block_types: &Arc<Vec<BlockTypeDescriptor>>,
        block: EditorBlock,
        data: &[u8],
        resync: bool,
    ) -> Vec<Message> {
        self.connect(context, client, client_id);
        if self.block_types.is_none() {
            self.block_types = Some(Arc::clone(block_types));
        }
        let entry = self.entries.entry(instance).or_insert_with(|| {
            let mut entry = Instance::new(context, InstanceRole::Artifact(block));
            entry.artifact.data = data.to_vec();
            entry
        });
        if entry.artifact.data == data && !resync {
            return Vec::new();
        }
        entry.artifact.data = data.to_vec();
        entry.artifact.draft = None;
        if !entry.opened {
            return Vec::new();
        }
        vec![Message::Editor(EditorMessage::ArtifactSettings {
            instance,
            data: data.to_vec(),
        })]
    }

    pub(super) fn artifact_description(
        &self,
        instance: EditorInstanceId,
    ) -> Option<ArtifactDescription> {
        self.entries.get(&instance)?.artifact.description.clone()
    }

    pub(super) fn artifact_draft(&self, instance: EditorInstanceId) -> Option<Vec<u8>> {
        self.entries.get(&instance)?.artifact.draft.clone()
    }

    pub(super) fn regenerate_artifact(
        &mut self,
        instance: EditorInstanceId,
        data: &[u8],
    ) -> Vec<Message> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        entry.artifact.outcome = None;
        vec![Message::Editor(EditorMessage::RegenerateArtifact {
            instance,
            data: data.to_vec(),
        })]
    }

    pub(super) fn take_artifact_outcome(
        &mut self,
        instance: EditorInstanceId,
    ) -> Option<Result<(), String>> {
        self.entries.get_mut(&instance)?.artifact.outcome.take()
    }

    pub(super) fn allow_network(&mut self, hosts: Vec<String>) {
        self.network = hosts;
    }

    pub(super) fn reopen(&mut self) {
        self.sent_block_types = false;
        self.announced.clear();
        for entry in self.entries.values_mut() {
            entry.opened = false;
            entry.reported_view = None;
            entry.reported_presenting = false;
            entry.fetches.clear();
            entry.assets.clear();
        }
    }

    pub(super) fn next_screens(&mut self, pass: u64) -> NextScreens {
        let client = self
            .connection
            .as_ref()
            .map(|connection| (Arc::clone(&connection.client), connection.client_id));
        let mut instances: Vec<_> = self.entries.keys().copied().collect();
        instances.sort_by_key(|instance| instance.0);
        let mut opened = Vec::new();
        let mut screens = Vec::new();
        if !self.sent_block_types {
            if let Some(block_types) = &self.block_types {
                self.sent_block_types = true;
                opened.push(Message::BlockTypes(block_types.as_ref().clone()));
            }
        }
        for instance in instances {
            let Some(entry) = self.entries.get_mut(&instance) else {
                continue;
            };
            let mut regions: Vec<_> = entry
                .screens
                .values()
                .filter(|screen| {
                    screen.last_seen >= pass
                        && screen.request.metrics.pixel_width > 0
                        && screen.request.metrics.pixel_height > 0
                })
                .map(|screen| screen.request.clone())
                .collect();
            if regions.is_empty() && matches!(entry.role, InstanceRole::Editor(_)) {
                continue;
            }
            regions.sort_by_key(|request| request.screen.0);
            let Some((client, client_id)) = &client else {
                continue;
            };
            let client_id = client_id.into_bytes();
            if !entry.opened {
                entry.opened = true;
                let account_id = client.account_id().into_bytes();
                let workspace_id = client.workspace_id().into_bytes();
                opened.push(match entry.role {
                    InstanceRole::Editor(block) => Message::Editor(EditorMessage::Open {
                        instance,
                        block_id: block.id.into_bytes(),
                        block_type: block.block_type.into_bytes(),
                        account_id,
                        workspace_id,
                        client_id,
                        editable: client.block_access(block.id) == block::BlockAccess::Edit,
                    }),
                    InstanceRole::Creation => Message::Editor(EditorMessage::OpenCreation {
                        instance,
                        account_id,
                        workspace_id,
                        client_id,
                    }),
                    InstanceRole::Artifact(block) => Message::Editor(EditorMessage::OpenArtifact {
                        instance,
                        block_id: block.id.into_bytes(),
                        block_type: block.block_type.into_bytes(),
                        account_id,
                        workspace_id,
                        client_id,
                        data: entry.artifact.data.clone(),
                    }),
                });
            }
            if entry.presenting != entry.reported_presenting {
                entry.reported_presenting = entry.presenting;
                opened.push(Message::Editor(EditorMessage::PresentingChanged {
                    instance,
                    presenting: entry.presenting,
                }));
            }
            if entry.view != entry.reported_view {
                entry.reported_view = entry.view;
                if let Some(view) = entry.view {
                    opened.push(Message::Editor(EditorMessage::ViewChanged {
                        instance,
                        x: view.rect.min.x,
                        y: view.rect.min.y,
                        width: view.rect.width(),
                        height: view.rect.height(),
                        scale: view.scale,
                    }));
                }
            }
            screens.extend(regions);
        }
        NextScreens { opened, screens }
    }

    pub(super) fn set_region_sizes(&mut self, sizes: Vec<RegionSize>) -> bool {
        let mut changed = false;
        for size in sizes {
            for entry in self.entries.values_mut() {
                if let Some(screen) = entry
                    .screens
                    .values_mut()
                    .find(|screen| screen.request.screen == size.screen)
                {
                    let used = egui::vec2(size.logical_width, size.logical_height);
                    changed |= screen.used != Some(used);
                    screen.used = Some(used);
                }
            }
        }
        changed
    }

    pub(super) fn drag(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        event: Option<BlockDragEvent>,
    ) -> Vec<Message> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        let Some(screen) = entry.screens.get_mut(&region) else {
            return Vec::new();
        };
        match event {
            Some(event) => {
                screen.dragging = !event.dropped;
                if event.dropped {
                    entry.drag_accepted = false;
                }
                vec![Message::Editor(EditorMessage::DragOver {
                    instance,
                    region,
                    x: event.position.x,
                    y: event.position.y,
                    block_id: event.block_id.into_bytes(),
                    block_type: event.block_type.into_bytes(),
                    dropped: event.dropped,
                })]
            }
            None if screen.dragging => {
                screen.dragging = false;
                entry.drag_accepted = false;
                vec![Message::Editor(EditorMessage::DragLeft { instance })]
            }
            None => Vec::new(),
        }
    }

    pub(super) fn file_drop(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        event: Option<FileDropEvent>,
    ) -> Vec<Message> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        let Some(screen) = entry.screens.get_mut(&region) else {
            return Vec::new();
        };
        match event {
            Some(event) => {
                screen.file_dropping = !event.dropped;
                vec![Message::Editor(EditorMessage::FileDrop {
                    instance,
                    region,
                    x: event.position.x,
                    y: event.position.y,
                    files: event.files,
                    dropped: event.dropped,
                })]
            }
            None if screen.file_dropping => {
                screen.file_dropping = false;
                vec![Message::Editor(EditorMessage::FileDropLeft { instance })]
            }
            None => Vec::new(),
        }
    }

    pub(super) fn drag_accepted(&self, instance: EditorInstanceId) -> bool {
        self.entries
            .get(&instance)
            .is_some_and(|entry| entry.drag_accepted)
    }

    pub(super) fn set_children(&mut self, placements: ChildPlacements) -> (Vec<Message>, bool) {
        let ChildPlacements {
            instance,
            region,
            generation,
            children,
            occluders,
        } = placements;
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
        else {
            return (Vec::new(), false);
        };
        screen.revoked.retain(|child| {
            children
                .iter()
                .any(|placement| placement.child == *child && placement.mode == ChildMode::Active)
        });
        screen.frame_revoked.retain(|child| {
            children.iter().any(|placement| {
                placement.child == *child
                    && matches!(placement.mode, ChildMode::Active | ChildMode::Live)
            })
        });
        let table = ChildTable {
            generation,
            size: egui::vec2(
                screen.request.metrics.logical_width,
                screen.request.metrics.logical_height,
            ),
            children,
            occluders,
        };
        let changed = screen.children != table;
        screen.children = table;
        (Vec::new(), changed)
    }

    pub(super) fn host_children(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
        rect: egui::Rect,
        clip: egui::Rect,
    ) -> (Vec<HostChild>, Holes) {
        let Some(screen) = self
            .entries
            .get(&instance)
            .and_then(|entry| entry.screens.get(&region))
        else {
            return (Vec::new(), Holes::default());
        };
        let table = &screen.children;
        let origin = rect.min.to_vec2();
        let stretch = egui::vec2(
            ratio(rect.width(), table.size.x),
            ratio(rect.height(), table.size.y),
        );
        let mut children = Vec::new();
        let mut holes = Holes::default();
        let mut live = 0;
        if let Some(report) = &screen.report {
            let painted: Vec<egui::Rect> = report
                .painted
                .iter()
                .map(|painted| host_rect(*painted, origin, stretch))
                .collect();
            if !painted.is_empty() {
                for piece in pieces::subtract(rect.intersect(clip), &painted) {
                    holes.holes.push(Hole {
                        rect: piece,
                        occluders: Vec::new(),
                    });
                }
            }
        }
        for (index, child) in table.children.iter().enumerate() {
            if child.rect.is_empty() {
                continue;
            }
            let requested = match (region, child.mode) {
                (EditorRegion::Preview, _) => ChildMode::Preview,
                (_, ChildMode::Active) if screen.revoked.contains(&child.child) => {
                    ChildMode::Passive
                }
                (_, mode) => mode,
            };
            let mode = match requested {
                ChildMode::Preview => ChildMode::Preview,
                mode => {
                    live += 1;
                    match live > MAX_LIVE_CHILDREN {
                        true => ChildMode::Preview,
                        false => mode,
                    }
                }
            };
            let child_rect = host_rect(child.rect, origin, stretch);
            let child_clip = host_rect(child.clip, origin, stretch).intersect(clip);
            if matches!(mode, ChildMode::Active | ChildMode::Live) {
                let interactive = child_rect.intersect(child_clip);
                if interactive.is_positive() {
                    holes.holes.push(Hole {
                        rect: interactive,
                        occluders: table
                            .occluders
                            .iter()
                            .filter(|occluder| occluder.after as usize > index)
                            .map(|occluder| host_rect(occluder.rect, origin, stretch))
                            .collect(),
                    });
                }
            }
            children.push(HostChild {
                child: child.child,
                frame_owner: matches!(mode, ChildMode::Active | ChildMode::Live)
                    && !screen.frame_revoked.contains(&child.child),
                block_id: Uuid::from_bytes(child.block_id),
                block_type: Uuid::from_bytes(child.block_type),
                rect: child_rect,
                clip: child_clip,
                layer: child.layer,
                mode,
                intrinsic: (child.intrinsic_width > 0.0 && child.intrinsic_height > 0.0)
                    .then(|| egui::vec2(child.intrinsic_width, child.intrinsic_height)),
                rotation: child.rotation,
                opacity: child.opacity,
            });
        }
        (children, holes)
    }

    pub(super) fn revoke_active(&mut self, instance: EditorInstanceId, region: EditorRegion) {
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
        else {
            return;
        };
        for child in &screen.children.children {
            if child.mode == ChildMode::Active {
                screen.revoked.insert(child.child);
            }
        }
    }

    pub(super) fn take_leaving(&mut self, instance: EditorInstanceId) -> bool {
        self.entries
            .get_mut(&instance)
            .is_some_and(|entry| std::mem::take(&mut entry.leaving))
    }

    pub(super) fn frame_child(&self, instance: EditorInstanceId) -> Option<Uuid> {
        let screen = self
            .entries
            .get(&instance)?
            .screens
            .get(&EditorRegion::Frame)?;
        screen
            .children
            .children
            .iter()
            .find(|child| {
                matches!(child.mode, ChildMode::Active | ChildMode::Live)
                    && !screen.frame_revoked.contains(&child.child)
                    && !child.rect.is_empty()
            })
            .map(|child| Uuid::from_bytes(child.block_id))
    }

    pub(super) fn revoke_frame_child(&mut self, instance: EditorInstanceId) {
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&EditorRegion::Frame))
        else {
            return;
        };
        for child in &screen.children.children {
            if matches!(child.mode, ChildMode::Active | ChildMode::Live) {
                screen.frame_revoked.insert(child.child);
            }
        }
    }

    pub(super) fn set_child_statuses(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        statuses: Vec<HostChildStatus>,
    ) -> Vec<Message> {
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
        else {
            return Vec::new();
        };
        let mut changed = Vec::new();
        let live: Vec<ChildId> = statuses.iter().map(|status| status.child).collect();
        for status in statuses {
            let status = ChildStatus {
                instance,
                region,
                child: status.child,
                available: status.available,
                intrinsic_width: status.intrinsic.map_or(0.0, |size| size.x),
                intrinsic_height: status.intrinsic.map_or(0.0, |size| size.y),
                aspect_ratio: status.aspect_ratio.unwrap_or_default(),
                hovered: status.hovered,
                active: status.active,
                interaction: status.interaction,
                capabilities: status.capabilities,
                resize: status.resize,
                error: status.error,
            };
            if screen.reported_statuses.get(&status.child) == Some(&status) {
                continue;
            }
            screen
                .reported_statuses
                .insert(status.child, status.clone());
            changed.push(status);
        }
        screen
            .reported_statuses
            .retain(|child, _| live.contains(child));
        match changed.is_empty() {
            true => Vec::new(),
            false => vec![Message::ChildStatuses(changed)],
        }
    }

    pub(super) fn take_block_pick(
        &mut self,
        instance: EditorInstanceId,
    ) -> Option<BlockPickRequest> {
        let entry = self.entries.get_mut(&instance)?;
        match entry.block_picks.is_empty() {
            true => None,
            false => Some(entry.block_picks.remove(0)),
        }
    }

    pub(super) fn block_picked(
        &self,
        instance: EditorInstanceId,
        request_id: u64,
        pick: BlockPick,
    ) -> Vec<Message> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Message::Editor(EditorMessage::BlockPicked {
            instance,
            request_id,
            pick,
        })]
    }

    pub(super) fn statuses(&self, layout: &ScreenLayout, pass: u64) -> Vec<super::InstanceStatus> {
        let mut statuses: Vec<_> = self
            .entries
            .iter()
            .map(|(instance, entry)| {
                let mut screens: Vec<_> = entry
                    .screens
                    .values()
                    .map(|screen| {
                        let metrics = &screen.request.metrics;
                        let placement = layout
                            .screens
                            .iter()
                            .find(|placement| placement.screen == screen.request.screen)
                            .map(|placement| {
                                [placement.x, placement.y, placement.width, placement.height]
                            });
                        super::ScreenStatus {
                            screen: screen.request.screen,
                            region: screen.request.region,
                            logical: egui::vec2(metrics.logical_width, metrics.logical_height),
                            pixels: [metrics.pixel_width, metrics.pixel_height],
                            scale_factor: metrics.scale_factor,
                            used: screen.used,
                            placement,
                            drawn: screen.last_seen >= pass,
                            children: screen.children.children.len(),
                            child_generation: screen.children.generation,
                        }
                    })
                    .collect();
                screens.sort_by_key(|screen| screen.screen.0);
                super::InstanceStatus {
                    instance: *instance,
                    block: entry.role.block().map(|block| block.id),
                    role: match entry.role {
                        InstanceRole::Editor(_) => "editor",
                        InstanceRole::Creation => "creation",
                        InstanceRole::Artifact(_) => "artifact",
                    },
                    opened: entry.opened,
                    aspect_ratio: entry.aspect_ratio,
                    intrinsic: entry.intrinsic,
                    view: entry.view.map(|view| view.rect),
                    artifact: matches!(entry.role, InstanceRole::Artifact(_)).then(|| {
                        super::ArtifactStatus {
                            data: entry.artifact.data.len(),
                            draft: entry.artifact.draft.as_ref().map(Vec::len),
                            description: entry.artifact.description.as_ref().map(|description| {
                                match description {
                                    ArtifactDescription::Described { summary, .. } => {
                                        summary.clone()
                                    }
                                    ArtifactDescription::Unreadable(error) => {
                                        format!("unreadable: {error}")
                                    }
                                }
                            }),
                        }
                    }),
                    screens,
                }
            })
            .collect();
        statuses.sort_by_key(|status| status.instance.0);
        statuses
    }

    pub(super) fn creation_ready(&self, instance: EditorInstanceId) -> bool {
        self.entries
            .get(&instance)
            .is_some_and(|entry| entry.creation_ready)
    }

    pub(super) fn commit_creation(&self, instance: EditorInstanceId) -> Vec<Message> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Message::Editor(EditorMessage::CommitCreation { instance })]
    }

    pub(super) fn take_created(
        &mut self,
        instance: EditorInstanceId,
    ) -> Option<Result<Uuid, String>> {
        self.entries.get_mut(&instance)?.created.take()
    }

    pub(super) fn aspect_ratio(&self, instance: EditorInstanceId) -> Option<f32> {
        self.entries.get(&instance)?.aspect_ratio
    }

    pub(super) fn intrinsic_size(&self, instance: EditorInstanceId) -> Option<egui::Vec2> {
        self.entries.get(&instance)?.intrinsic
    }

    pub(super) fn region_size(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<egui::Vec2> {
        self.entries.get(&instance)?.screens.get(&region)?.used
    }

    pub(super) fn set_frame_reports(&mut self, reports: Vec<FrameReport>) -> bool {
        let mut changed = false;
        for report in reports {
            for entry in self.entries.values_mut() {
                if let Some(screen) = entry
                    .screens
                    .values_mut()
                    .find(|screen| screen.request.screen == report.screen)
                {
                    changed |= screen.report.as_ref() != Some(&report);
                    screen.report = Some(report.clone());
                }
            }
        }
        changed
    }

    pub(super) fn frame_report(&self, instance: EditorInstanceId) -> Option<&FrameReport> {
        self.entries
            .get(&instance)?
            .screens
            .get(&EditorRegion::Frame)?
            .report
            .as_ref()
    }

    pub(super) fn drive_web_views(
        &mut self,
        frame: &eframe::Frame,
        context: &egui::Context,
        pass: u64,
    ) -> Vec<Message> {
        let mut messages = Vec::new();
        for (instance, entry) in &mut self.entries {
            if entry.web_view.is_none() {
                continue;
            }
            let rect = entry.web_view_rect.and_then(|(region, rect)| {
                let screen = entry.screens.get(&region)?;
                let placement = screen.placement?;
                let live = placement.pass == pass && screen.last_seen == pass;
                let origin = placement.rect.min.to_vec2();
                let stretch = egui::vec2(
                    ratio(placement.rect.width(), screen.request.metrics.logical_width),
                    ratio(
                        placement.rect.height(),
                        screen.request.metrics.logical_height,
                    ),
                );
                live.then(|| host_rect(rect, origin, stretch).intersect(placement.clip))
            });
            let mut events = Vec::new();
            let view = entry.web_view.as_mut().expect("the web view is present");
            view.drive(frame, context, rect, &mut events);
            for event in events {
                messages.push(Message::Editor(EditorMessage::WebViewEvent {
                    instance: *instance,
                    event,
                }));
            }
        }
        messages
    }

    pub(super) fn presence(
        &mut self,
        instance: EditorInstanceId,
        visible: bool,
        entries: Vec<PresenceEntry>,
    ) -> Vec<Message> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        if entry
            .presence
            .as_ref()
            .is_some_and(|(shown, seen)| *shown == visible && *seen == entries)
        {
            return Vec::new();
        }
        entry.presence = Some((visible, entries.clone()));
        vec![Message::Editor(EditorMessage::Presence {
            instance,
            visible,
            entries,
        })]
    }

    pub(super) fn child_view_changes(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        changes: Vec<(ChildId, ViewChange)>,
    ) -> Vec<Message> {
        changes
            .into_iter()
            .map(|(child, change)| {
                Message::Editor(EditorMessage::ChildView {
                    instance,
                    region,
                    child,
                    change,
                })
            })
            .collect()
    }

    pub(super) fn take_presence_publications(
        &mut self,
        instance: EditorInstanceId,
    ) -> Vec<PresencePublication> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.presence_publications))
            .unwrap_or_default()
    }

    pub(super) fn replace_child(
        &mut self,
        instance: EditorInstanceId,
        old: Uuid,
        new: Uuid,
    ) -> (Vec<Message>, Option<bool>) {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return (Vec::new(), None);
        };
        match entry.replacements.get(&(old, new)) {
            Some(Replacement::Pending(_)) => (Vec::new(), None),
            Some(Replacement::Answered(_)) => {
                let Some(Replacement::Answered(replaced)) = entry.replacements.remove(&(old, new))
                else {
                    unreachable!("the replacement was just answered")
                };
                (Vec::new(), Some(replaced))
            }
            None => {
                entry.next_replacement += 1;
                let request_id = entry.next_replacement;
                entry
                    .replacements
                    .insert((old, new), Replacement::Pending(request_id));
                (
                    vec![Message::Editor(EditorMessage::ReplaceChild {
                        instance,
                        request_id,
                        old: old.into_bytes(),
                        new: new.into_bytes(),
                    })],
                    None,
                )
            }
        }
    }

    pub(super) fn grabbing(&self) -> bool {
        self.entries.values().any(|entry| entry.grabbed)
    }

    pub(super) fn ime(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
        rect: egui::Rect,
    ) -> Option<egui::output::IMEOutput> {
        let screen = self.entries.get(&instance)?.screens.get(&region)?;
        let area = screen.ime?;
        let origin = rect.min.to_vec2();
        let stretch = egui::vec2(
            ratio(rect.width(), screen.request.metrics.logical_width),
            ratio(rect.height(), screen.request.metrics.logical_height),
        );
        Some(egui::output::IMEOutput {
            rect: host_rect(area.rect, origin, stretch),
            cursor_rect: host_rect(area.cursor, origin, stretch),
        })
    }

    pub(super) fn cursor(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<egui::CursorIcon> {
        let screen = self.entries.get(&instance)?.screens.get(&region)?;
        Some(match screen.cursor {
            CursorIcon::Default => egui::CursorIcon::Default,
            CursorIcon::None => egui::CursorIcon::None,
            CursorIcon::Pointer => egui::CursorIcon::PointingHand,
            CursorIcon::Text => egui::CursorIcon::Text,
            CursorIcon::Crosshair => egui::CursorIcon::Crosshair,
            CursorIcon::Grab => egui::CursorIcon::Grab,
            CursorIcon::Grabbing => egui::CursorIcon::Grabbing,
            CursorIcon::Move => egui::CursorIcon::Move,
            CursorIcon::NotAllowed => egui::CursorIcon::NotAllowed,
            CursorIcon::Wait => egui::CursorIcon::Wait,
            CursorIcon::Progress => egui::CursorIcon::Progress,
            CursorIcon::Help => egui::CursorIcon::Help,
            CursorIcon::ResizeHorizontal => egui::CursorIcon::ResizeHorizontal,
            CursorIcon::ResizeVertical => egui::CursorIcon::ResizeVertical,
            CursorIcon::ResizeNeSw => egui::CursorIcon::ResizeNeSw,
            CursorIcon::ResizeNwSe => egui::CursorIcon::ResizeNwSe,
        })
    }

    pub(super) fn screen_set(&mut self, screens: Vec<ScreenRequest>) -> Message {
        self.announced = screens.iter().map(|screen| screen.screen).collect();
        self.request_id += 1;
        Message::Screens(ScreenSet {
            request_id: self.request_id,
            screens,
        })
    }

    pub(super) fn place(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        placement: Placement,
    ) {
        if let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
        {
            screen.placement = Some(placement);
        }
    }

    pub(super) fn frame_input(&mut self, context: &egui::Context, pass: u64) -> Vec<Message> {
        let announced = &self.announced;
        let mut placed: Vec<_> = self
            .entries
            .iter()
            .flat_map(|(instance, entry)| {
                entry.screens.iter().filter_map(move |(region, screen)| {
                    let placement = screen.placement?;
                    let live = placement.pass == pass
                        && screen.last_seen == pass
                        && announced.contains(&screen.request.screen);
                    live.then_some((*instance, *region, screen.request.screen, placement))
                })
            })
            .collect();
        placed.sort_by_key(|(instance, _, screen, _)| (instance.0, screen.0));
        let mut messages = Vec::new();
        for (instance, region, screen, placement) in placed {
            let (_, holes) = self.host_children(instance, region, placement.rect, placement.clip);
            let Some(response) = context.read_response(placement.id) else {
                continue;
            };
            if response.clicked() {
                response.request_focus();
            }
            let focused = response.has_focus();
            let hovered = response.hovered();
            messages.extend(self.input(instance, region, |input| {
                input.update(context, placement.rect, hovered, focused, screen, &holes)
            }));
            let over_hole = context
                .pointer_latest_pos()
                .is_some_and(|position| holes.contains(position));
            let dismissed = context.input(|input| {
                input.key_pressed(egui::Key::Escape)
                    || (input.pointer.button_pressed(egui::PointerButton::Primary) && !over_hole)
            });
            if dismissed {
                self.revoke_active(instance, region);
            }
        }
        messages
    }

    fn input(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        update: impl FnOnce(&mut InputAdapter) -> Vec<Message>,
    ) -> Vec<Message> {
        self.entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
            .map(|screen| update(&mut screen.input))
            .unwrap_or_default()
    }

    pub(super) fn client_responses(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        if let Some(connection) = &mut self.connection {
            while let Some(payload) = connection.tunnel.try_recv() {
                messages.push(Message::Client(TunnelMessage::Response { payload }));
            }
        }
        messages
    }

    pub(super) fn pending(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        let mut instances: Vec<_> = self.entries.keys().copied().collect();
        instances.sort_by_key(|instance| instance.0);
        for instance in instances {
            let entry = self.entries.get_mut(&instance).unwrap();
            let context = entry.context.clone();
            let mut picks = std::mem::take(&mut entry.picks);
            picks.retain_mut(|pending| {
                let pick = match pending.picker.poll(&context) {
                    Some(Ok(file)) => FilePick::Chosen {
                        name: file.name,
                        data: file.data,
                    },
                    Some(Err(error)) => FilePick::Failed(error),
                    None if pending.picker.is_open() => return true,
                    None => FilePick::Cancelled,
                };
                messages.push(Message::Editor(EditorMessage::FilePicked {
                    instance,
                    request_id: pending.request_id,
                    pick,
                }));
                false
            });
            self.entries.get_mut(&instance).unwrap().picks = picks;
            let entry = self.entries.get_mut(&instance).unwrap();
            entry.fetches.retain(|pending| {
                let Some(result) = pending.fetch.poll() else {
                    context.request_repaint_after(FETCH_POLL_INTERVAL);
                    return true;
                };
                messages.push(Message::Editor(EditorMessage::Fetched {
                    instance,
                    request_id: pending.request_id,
                    result: match result {
                        Ok(body) => FetchResult::Body(body),
                        Err(error) => FetchResult::Failed(error),
                    },
                }));
                false
            });
            let entry = self.entries.get_mut(&instance).unwrap();
            entry.assets.retain_mut(|pending| {
                let Some(result) = pending.asset.poll() else {
                    context.request_repaint_after(FETCH_POLL_INTERVAL);
                    return true;
                };
                messages.push(Message::Editor(EditorMessage::AssetRead {
                    instance,
                    request_id: pending.request_id,
                    result: match result {
                        Ok(body) => AssetResult::Body(body),
                        Err(error) => AssetResult::Failed(error),
                    },
                }));
                false
            });
            let entry = self.entries.get_mut(&instance).unwrap();
            for pending in std::mem::take(&mut entry.pastes) {
                messages.push(Message::Editor(EditorMessage::ImagePasted {
                    instance,
                    request_id: pending.request_id,
                    image: pending.image,
                }));
            }
            let entry = self.entries.get_mut(&instance).unwrap();
            if let Some(player) = &entry.audio {
                let status = player.status();
                if status.playing {
                    context.request_repaint();
                }
                if status != entry.reported_audio {
                    entry.reported_audio.clone_from(&status);
                    messages.push(Message::Editor(EditorMessage::AudioStatus {
                        instance,
                        status,
                    }));
                }
            }
        }
        messages
    }

    pub(super) fn editor_message(&mut self, message: EditorMessage) -> bool {
        match message {
            EditorMessage::OpenBlock {
                instance,
                block_id,
                block_type,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry
                    .opens
                    .push((Uuid::from_bytes(block_id), Uuid::from_bytes(block_type)));
                true
            }
            EditorMessage::DragAccepted { instance, accepted } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let changed = entry.drag_accepted != accepted;
                entry.drag_accepted = accepted;
                changed
            }
            EditorMessage::PickFile {
                instance,
                request_id,
                filter,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let mut picker = FilePicker::default();
                picker.open(&entry.context, &host_filter(filter));
                entry.picks.push(PendingPick { request_id, picker });
                true
            }
            EditorMessage::PasteImage {
                instance,
                request_id,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.pastes.push(PendingPaste {
                    request_id,
                    image: super::clipboard::read_clipboard_image(),
                });
                true
            }
            EditorMessage::PlayAudio {
                instance,
                block_id,
                command,
            } => {
                let Some(client) = self
                    .connection
                    .as_ref()
                    .map(|connection| Arc::clone(&connection.client))
                else {
                    return false;
                };
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let player = entry.audio.get_or_insert_with(AudioPlayer::new);
                match command {
                    AudioCommand::Reset => player.reset(),
                    AudioCommand::Toggle => {
                        let block = client.get_block::<Audio>(Uuid::from_bytes(block_id));
                        let audio = block.read().map(|audio| audio.clone());
                        if let Some(audio) = audio {
                            player.toggle(&audio);
                        }
                    }
                }
                true
            }
            EditorMessage::Fetch {
                instance,
                request_id,
                url,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let fetch = match allowed(&url, &self.network) {
                    true => Fetch::get(url, Vec::new()),
                    false => Fetch::refused(format!("{REFUSED} {url}")),
                };
                entry.fetches.push(PendingFetch { request_id, fetch });
                true
            }
            EditorMessage::WebView {
                instance,
                region,
                rect,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.web_view_rect = rect.map(|rect| (region, rect));
                true
            }
            EditorMessage::WebViewCommand { instance, command } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.web_view.get_or_insert_default().command(command);
                true
            }
            EditorMessage::GrabCursor { instance, grabbed } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.grabbed = grabbed;
                true
            }
            EditorMessage::ReadAsset {
                instance,
                request_id,
                name,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.assets.push(PendingAsset {
                    request_id,
                    asset: Asset::read(&name),
                });
                true
            }
            EditorMessage::PickBlock {
                instance,
                request_id,
                filter,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.block_picks.push(BlockPickRequest {
                    request_id,
                    block_types: filter
                        .block_types
                        .into_iter()
                        .map(Uuid::from_bytes)
                        .collect(),
                    templates: filter.templates,
                });
                true
            }
            EditorMessage::CreationReady { instance, ready } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let changed = entry.creation_ready != ready;
                entry.creation_ready = ready;
                changed
            }
            EditorMessage::CreationBlock { instance, outcome } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.created = Some(match outcome {
                    CreationOutcome::Created(block_id) => Ok(Uuid::from_bytes(block_id)),
                    CreationOutcome::Failed(error) => Err(error),
                });
                true
            }
            EditorMessage::Ime {
                instance,
                region,
                area,
            } => {
                let Some(screen) = self
                    .entries
                    .get_mut(&instance)
                    .and_then(|entry| entry.screens.get_mut(&region))
                else {
                    return false;
                };
                let changed = screen.ime != area;
                screen.ime = area;
                changed
            }
            EditorMessage::CopyText { instance, text } => {
                let Some(entry) = self.entries.get(&instance) else {
                    return false;
                };
                entry.context.copy_text(text);
                false
            }
            EditorMessage::PublishPresence {
                instance,
                presence_id,
                data,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry
                    .presence_publications
                    .push((Uuid::from_bytes(presence_id), data));
                false
            }
            EditorMessage::ChildReplaced {
                instance,
                request_id,
                replaced,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let Some(key) = entry
                    .replacements
                    .iter()
                    .find(
                        |(_, state)| matches!(state, Replacement::Pending(id) if *id == request_id),
                    )
                    .map(|(key, _)| *key)
                else {
                    return false;
                };
                entry
                    .replacements
                    .insert(key, Replacement::Answered(replaced));
                true
            }
            EditorMessage::Cursor {
                instance,
                region,
                cursor,
            } => {
                let Some(screen) = self
                    .entries
                    .get_mut(&instance)
                    .and_then(|entry| entry.screens.get_mut(&region))
                else {
                    return false;
                };
                let changed = screen.cursor != cursor;
                screen.cursor = cursor;
                changed
            }
            EditorMessage::ArtifactDescribed {
                instance,
                description,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.artifact.description = Some(description);
                true
            }
            EditorMessage::ArtifactEdited { instance, data } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let changed = entry.artifact.draft.as_ref() != Some(&data);
                entry.artifact.draft = Some(data);
                changed
            }
            EditorMessage::ArtifactRegenerated { instance, outcome } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.artifact.outcome = Some(match outcome {
                    RegenerationOutcome::Done => Ok(()),
                    RegenerationOutcome::Failed(error) => Err(error),
                });
                true
            }
            EditorMessage::Present {
                instance,
                presenting,
            } => self.set_presenting(instance, presenting),
            EditorMessage::LeaveFrame { instance } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.leaving = true;
                true
            }
            EditorMessage::ChangeView { instance, change } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.view_changes.push(change);
                true
            }
            EditorMessage::AspectRatio { instance, ratio } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let changed = entry.aspect_ratio != Some(ratio);
                entry.aspect_ratio = Some(ratio);
                changed
            }
            EditorMessage::IntrinsicSize {
                instance,
                width,
                height,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let intrinsic = egui::vec2(width, height);
                let changed = entry.intrinsic != Some(intrinsic);
                entry.intrinsic = Some(intrinsic);
                changed
            }
            EditorMessage::Performance {
                instance,
                group,
                measurements,
            } if self.entries.contains_key(&instance) => {
                for measurement in measurements {
                    match measurement {
                        PerformanceMeasurement::Duration { name, nanoseconds } => {
                            performance::record_group_duration(
                                &group,
                                &name,
                                std::time::Duration::from_nanos(nanoseconds),
                            );
                        }
                        PerformanceMeasurement::Count { name, count } => {
                            performance::record_group_count(&group, &name, count);
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }

    pub(super) fn take_open(&mut self, instance: EditorInstanceId) -> Option<(Uuid, Uuid)> {
        let entry = self.entries.get_mut(&instance)?;
        if entry.opens.is_empty() {
            None
        } else {
            Some(entry.opens.remove(0))
        }
    }

    pub(super) fn client_message(&mut self, message: TunnelMessage) {
        let TunnelMessage::Request { payload } = message else {
            return;
        };
        let Some(connection) = &self.connection else {
            log(&format!(
                "dropped a plugin client frame: the runtime has no connection: {}",
                summary(&payload)
            ));
            return;
        };
        log(&format!(
            "forwarding a plugin client frame: {}",
            summary(&payload)
        ));
        connection.tunnel.send(payload);
    }
}

fn host_rect(
    rect: block_plugin_api::ChildRect,
    origin: egui::Vec2,
    stretch: egui::Vec2,
) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(rect.x * stretch.x, rect.y * stretch.y) + origin,
        egui::vec2(rect.width * stretch.x, rect.height * stretch.y),
    )
}

fn ratio(current: f32, published: f32) -> f32 {
    match published > 0.0 && current > 0.0 {
        true => current / published,
        false => 1.0,
    }
}

fn summary(payload: &str) -> String {
    const LONGEST: usize = 160;
    match payload.char_indices().nth(LONGEST) {
        Some((end, _)) => format!("{}...", &payload[..end]),
        None => payload.to_owned(),
    }
}

fn log(message: &str) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&format!("plugin host {message}").into());
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("plugin host {message}");
}

fn host_filter(filter: block_plugin_api::FileFilter) -> FileFilter {
    FileFilter {
        name: filter.name,
        default_file_name: filter.default_file_name,
        extensions: filter.extensions,
        mime_types: filter.mime_types,
    }
}

fn allowed(url: &str, hosts: &[String]) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    hosts.iter().any(|allowed| allowed == host)
}

#[cfg(test)]
mod tests;
