use block_client::{BlockClient, Tunnel};
use block_plugin_api::{
    BlockTypeDescriptor, CreationOutcome, EditorInstanceId, EditorMessage, EditorRegion, FilePick,
    Message, RegionSize, ScreenId, ScreenRequest, ScreenSet, TunnelMessage,
};
use eframe::egui;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

use super::{
    input::{viewport_metrics, BlockDragEvent, InputAdapter},
    EditorBlock,
};
use crate::platform::{FileFilter, FilePicker};

#[derive(Default)]
pub(super) struct Instances {
    entries: HashMap<EditorInstanceId, Instance>,
    next_screen: u64,
    request_id: u64,
    block_types: Option<Arc<Vec<BlockTypeDescriptor>>>,
    sent_block_types: bool,
}

struct Instance {
    context: egui::Context,
    client: Arc<BlockClient>,
    tunnel: Tunnel,
    block: Option<EditorBlock>,
    creation_ready: bool,
    created: Option<Result<Uuid, String>>,
    screens: HashMap<EditorRegion, Screen>,
    opened: bool,
    opens: Vec<(Uuid, Uuid)>,
    drag_accepted: bool,
    intrinsic: Option<egui::Vec2>,
    aspect_ratio: Option<f32>,
    picks: Vec<PendingPick>,
}

struct PendingPick {
    request_id: u64,
    picker: FilePicker,
}

struct Screen {
    input: InputAdapter,
    request: ScreenRequest,
    last_seen: u64,
    used: Option<egui::Vec2>,
    dragging: bool,
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

    pub(super) fn report(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        context: &egui::Context,
        client: &Arc<BlockClient>,
        block: Option<EditorBlock>,
        block_types: &Arc<Vec<BlockTypeDescriptor>>,
        size: egui::Vec2,
        scale_factor: f32,
        pass: u64,
    ) -> ScreenId {
        if self.block_types.is_none() {
            self.block_types = Some(Arc::clone(block_types));
        }
        let entry = self.entries.entry(instance).or_insert_with(|| {
            let tunnel = client.open_tunnel({
                let context = context.clone();
                move || context.request_repaint()
            });
            Instance {
                context: context.clone(),
                client: Arc::clone(client),
                tunnel,
                block,
                creation_ready: false,
                created: None,
                screens: HashMap::new(),
                opened: false,
                opens: Vec::new(),
                drag_accepted: false,
                intrinsic: None,
                aspect_ratio: None,
                picks: Vec::new(),
            }
        });
        let next_screen = &mut self.next_screen;
        let screen = entry.screens.entry(region).or_insert_with(|| {
            *next_screen += 1;
            Screen {
                input: InputAdapter::default(),
                request: ScreenRequest {
                    screen: ScreenId(*next_screen),
                    instance,
                    region,
                    metrics: viewport_metrics(size, scale_factor),
                },
                last_seen: pass,
                used: None,
                dragging: false,
            }
        });
        screen.request.metrics = viewport_metrics(size, scale_factor);
        screen.last_seen = pass;
        screen.request.screen
    }

    pub(super) fn next_screens(&mut self, pass: u64) -> NextScreens {
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
            if regions.is_empty() {
                continue;
            }
            regions.sort_by_key(|request| request.screen.0);
            if !entry.opened {
                entry.opened = true;
                let account_id = entry.client.account_id().into_bytes();
                let workspace_id = entry.client.workspace_id().into_bytes();
                opened.push(match &entry.block {
                    Some(block) => Message::Editor(EditorMessage::Open {
                        instance,
                        block_id: block.id.into_bytes(),
                        block_type: block.block_type.into_bytes(),
                        account_id,
                        workspace_id,
                        editable: entry.client.block_access(block.id) == block::BlockAccess::Edit,
                    }),
                    None => Message::Editor(EditorMessage::OpenCreation {
                        instance,
                        account_id,
                        workspace_id,
                    }),
                });
            }
            screens.extend(regions);
        }
        NextScreens { opened, screens }
    }

    pub(super) fn set_region_sizes(&mut self, sizes: Vec<RegionSize>) {
        for size in sizes {
            for entry in self.entries.values_mut() {
                if let Some(screen) = entry
                    .screens
                    .values_mut()
                    .find(|screen| screen.request.screen == size.screen)
                {
                    screen.used = Some(egui::vec2(size.logical_width, size.logical_height));
                }
            }
        }
    }

    /// Tells an instance about a block being dragged over one of its
    /// regions, and about a drag that has moved off it again.
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

    pub(super) fn statuses(&self) -> Vec<super::InstanceStatus> {
        let mut statuses: Vec<_> = self
            .entries
            .iter()
            .map(|(instance, entry)| {
                let mut regions: Vec<_> = entry.screens.keys().copied().collect();
                regions.sort_by_key(|region| format!("{region:?}"));
                super::InstanceStatus {
                    instance: *instance,
                    block: entry.block.map(|block| block.id),
                    regions,
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

    pub(super) fn screen_set(&mut self, screens: Vec<ScreenRequest>) -> Message {
        self.request_id += 1;
        Message::Screens(ScreenSet {
            request_id: self.request_id,
            screens,
        })
    }

    pub(super) fn input(
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

    /// Takes whatever the server has sent back for each instance's client, so
    /// it can be handed to the plugin runtime.
    pub(super) fn pending(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        let mut instances: Vec<_> = self.entries.keys().copied().collect();
        instances.sort_by_key(|instance| instance.0);
        for instance in instances {
            let entry = self.entries.get_mut(&instance).unwrap();
            while let Some(payload) = entry.tunnel.try_recv() {
                messages.push(Message::Client(TunnelMessage::Response {
                    instance,
                    payload,
                }));
            }
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
        }
        messages
    }

    /// Records an instance's request to have another block opened, to be
    /// picked up by the editor the next time it draws.
    pub(super) fn editor_message(&mut self, message: EditorMessage) {
        match message {
            EditorMessage::OpenBlock {
                instance,
                block_id,
                block_type,
            } => {
                if let Some(entry) = self.entries.get_mut(&instance) {
                    entry
                        .opens
                        .push((Uuid::from_bytes(block_id), Uuid::from_bytes(block_type)));
                }
            }
            EditorMessage::DragAccepted { instance, accepted } => {
                if let Some(entry) = self.entries.get_mut(&instance) {
                    entry.drag_accepted = accepted;
                }
            }
            EditorMessage::PickFile {
                instance,
                request_id,
                filter,
            } => {
                if let Some(entry) = self.entries.get_mut(&instance) {
                    let mut picker = FilePicker::default();
                    picker.open(&entry.context, &host_filter(filter));
                    entry.picks.push(PendingPick { request_id, picker });
                }
            }
            EditorMessage::CreationReady { instance, ready } => {
                if let Some(entry) = self.entries.get_mut(&instance) {
                    entry.creation_ready = ready;
                }
            }
            EditorMessage::CreationBlock { instance, outcome } => {
                if let Some(entry) = self.entries.get_mut(&instance) {
                    entry.created = Some(match outcome {
                        CreationOutcome::Created(block_id) => Ok(Uuid::from_bytes(block_id)),
                        CreationOutcome::Failed(error) => Err(error),
                    });
                }
            }
            EditorMessage::AspectRatio { instance, ratio } => {
                if let Some(entry) = self.entries.get_mut(&instance) {
                    entry.aspect_ratio = Some(ratio);
                }
            }
            EditorMessage::IntrinsicSize {
                instance,
                width,
                height,
            } => {
                if let Some(entry) = self.entries.get_mut(&instance) {
                    entry.intrinsic = Some(egui::vec2(width, height));
                }
            }
            _ => {}
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

    /// Forwards a plugin's client message to the server over the host's own
    /// connection, where it is served as a client in its own right.
    pub(super) fn client_message(&mut self, message: TunnelMessage) {
        let TunnelMessage::Request { instance, payload } = message else {
            return;
        };
        let Some(entry) = self.entries.get(&instance) else {
            return;
        };
        entry.tunnel.send(payload);
    }
}

fn host_filter(filter: block_plugin_api::FileFilter) -> FileFilter {
    FileFilter {
        name: filter.name,
        default_file_name: filter.default_file_name,
        extensions: filter.extensions,
        mime_types: filter.mime_types,
    }
}
