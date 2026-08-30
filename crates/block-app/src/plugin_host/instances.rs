use block_client::{BlockClient, Tunnel};
use block_plugin_api::{
    ArtifactDescription, BlockPick, BlockTypeDescriptor, ChildId, ChildMode, ChildPlacement,
    ChildPlacements, ChildStatus, CreationOutcome, CursorIcon, EditorInstanceId, EditorMessage,
    EditorRegion, FetchResult, FilePick, Message, Occluder, PerformanceMeasurement,
    RegenerationOutcome, RegionSize, ScreenId, ScreenLayout, ScreenRequest, ScreenSet,
    TunnelMessage, ViewChange,
};
use eframe::egui;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;

use super::{
    input::{viewport_metrics, BlockDragEvent, InputAdapter},
    BlockPickRequest, EditorBlock, HostChild, HostChildStatus, InstanceRole, MAX_LIVE_CHILDREN,
};
use crate::{
    performance,
    platform::{http::Fetch, FileFilter, FilePicker},
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
    block_picks: Vec<BlockPickRequest>,
    view: Option<egui::Rect>,
    reported_view: Option<egui::Rect>,
    view_changes: Vec<ViewChange>,
    presenting: bool,
    reported_presenting: bool,
    hidden_regions: HashSet<EditorRegion>,
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
            block_picks: Vec::new(),
            view: None,
            reported_view: None,
            view_changes: Vec::new(),
            presenting: false,
            reported_presenting: false,
            hidden_regions: HashSet::new(),
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
    dragging: bool,
    cursor: CursorIcon,
    children: ChildTable,
    reported_statuses: HashMap<ChildId, ChildStatus>,
    revoked: HashSet<ChildId>,
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
                },
                last_seen: pass,
                used: None,
                dragging: false,
                cursor: CursorIcon::Default,
                children: ChildTable::default(),
                reported_statuses: HashMap::new(),
                revoked: HashSet::new(),
            }
        });
        screen.request.metrics = viewport_metrics(size, visible, scale_factor);
        screen.last_seen = pass;
        screen.request.screen
    }

    pub(super) fn set_view(&mut self, instance: EditorInstanceId, view: egui::Rect) {
        if let Some(entry) = self.entries.get_mut(&instance) {
            entry.view = Some(view);
        }
    }

    pub(super) fn region_shown(&self, instance: EditorInstanceId, region: EditorRegion) -> bool {
        self.entries
            .get(&instance)
            .is_none_or(|entry| !entry.hidden_regions.contains(&region))
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
                        x: view.min.x,
                        y: view.min.y,
                        width: view.width(),
                        height: view.height(),
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
        for (index, child) in table.children.iter().enumerate() {
            if child.rect.is_empty() || (!child.part.is_main() && region == EditorRegion::Preview) {
                continue;
            }
            let requested = match (region, child.mode) {
                (EditorRegion::Preview, _) => ChildMode::Preview,
                (_, ChildMode::Active) if screen.revoked.contains(&child.child) => {
                    ChildMode::Passive
                }
                (_, mode) => mode,
            };
            let mode = match (child.part.is_main(), requested) {
                (true, ChildMode::Preview) => ChildMode::Preview,
                (true, mode) => {
                    live += 1;
                    match live > MAX_LIVE_CHILDREN {
                        true => ChildMode::Preview,
                        false => mode,
                    }
                }
                (false, _) => ChildMode::Live,
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
                block_id: Uuid::from_bytes(child.block_id),
                block_type: Uuid::from_bytes(child.block_type),
                rect: child_rect,
                clip: child_clip,
                layer: child.layer,
                mode,
                part: child.part,
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
                has_left_sidebar: status.has_left_sidebar,
                has_right_sidebar: status.has_right_sidebar,
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
                    view: entry.view,
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
            EditorMessage::ShowRegion {
                instance,
                region,
                shown,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                match shown {
                    true => entry.hidden_regions.remove(&region),
                    false => entry.hidden_regions.insert(region),
                }
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
