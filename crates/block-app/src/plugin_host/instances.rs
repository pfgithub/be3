use block::Block;
use block_client::{blocks::counter::Counter, BlockClient, BlockHandle};
use block_plugin_api::{
    DelegatedClientMessage, EditorInstanceId, EditorMessage, Message, ScreenId, ScreenRequest,
    ScreenSet,
};
use eframe::egui;
use std::{collections::HashMap, sync::Arc};

use super::input::{viewport_metrics, InputAdapter};

#[derive(Default)]
pub(super) struct Instances {
    entries: HashMap<EditorInstanceId, Instance>,
    next_screen: u64,
    request_id: u64,
}

struct Instance {
    client: Arc<BlockClient>,
    block: BlockHandle<Counter>,
    input: InputAdapter,
    screen: ScreenId,
    request: ScreenRequest,
    last_seen: u64,
    opened: bool,
    sequence: u64,
    pending_watch: Option<u64>,
}

pub(super) struct NextScreens {
    pub(super) opened: Vec<Message>,
    pub(super) screens: Vec<ScreenRequest>,
}

impl Instances {
    pub(super) fn block(&self, instance: EditorInstanceId) -> Option<BlockHandle<Counter>> {
        self.entries.get(&instance).map(|entry| entry.block.clone())
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn remove(&mut self, instance: EditorInstanceId) -> bool {
        self.entries.remove(&instance).is_some()
    }

    pub(super) fn report(
        &mut self,
        instance: EditorInstanceId,
        client: Arc<BlockClient>,
        block: BlockHandle<Counter>,
        size: egui::Vec2,
        scale_factor: f32,
        pass: u64,
    ) -> ScreenId {
        let next_screen = &mut self.next_screen;
        let entry = self.entries.entry(instance).or_insert_with(|| {
            *next_screen += 1;
            Instance {
                client,
                block,
                input: InputAdapter::default(),
                screen: ScreenId(*next_screen),
                request: ScreenRequest {
                    screen: ScreenId(*next_screen),
                    instance,
                    region: "main".into(),
                    metrics: viewport_metrics(size, scale_factor),
                },
                last_seen: pass,
                opened: false,
                sequence: 0,
                pending_watch: None,
            }
        });
        entry.request.metrics = viewport_metrics(size, scale_factor);
        entry.last_seen = pass;
        entry.screen
    }

    pub(super) fn next_screens(&mut self, pass: u64) -> NextScreens {
        let mut instances: Vec<_> = self.entries.keys().copied().collect();
        instances.sort_by_key(|instance| instance.0);
        let mut opened = Vec::new();
        let mut screens = Vec::new();
        for instance in instances {
            let Some(entry) = self.entries.get_mut(&instance) else {
                continue;
            };
            if entry.last_seen < pass {
                continue;
            }
            if !entry.opened {
                entry.opened = true;
                opened.push(Message::Editor(EditorMessage::Open {
                    instance,
                    block_id: entry.block.id().into_bytes(),
                    block_type: Counter::TYPE_ID.into_bytes(),
                    account_id: entry.client.account_id().into_bytes(),
                    workspace_id: entry.client.workspace_id().into_bytes(),
                    editable: entry.client.block_access(entry.block.id())
                        == block::BlockAccess::Edit,
                }));
            }
            if entry.request.metrics.pixel_width > 0 && entry.request.metrics.pixel_height > 0 {
                screens.push(entry.request.clone());
            }
        }
        NextScreens { opened, screens }
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
        update: impl FnOnce(&mut InputAdapter) -> Vec<Message>,
    ) -> Vec<Message> {
        self.entries
            .get_mut(&instance)
            .map(|entry| update(&mut entry.input))
            .unwrap_or_default()
    }

    pub(super) fn pending(&mut self) -> Vec<Message> {
        let instances: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.pending_watch.is_some())
            .map(|(instance, _)| *instance)
            .collect();
        let mut messages = Vec::new();
        for instance in instances {
            let Some(request_id) = self.entries[&instance].pending_watch else {
                continue;
            };
            if let Some(message) = self.snapshot(instance, request_id) {
                self.entries.get_mut(&instance).unwrap().pending_watch = None;
                messages.push(message);
            }
        }
        messages
    }

    pub(super) fn client_message(
        &mut self,
        message: DelegatedClientMessage,
        operate: impl FnOnce(&[u8]) -> bool,
    ) -> Vec<Message> {
        match message {
            DelegatedClientMessage::Watch {
                instance,
                request_id,
                ..
            } => match self.snapshot(instance, request_id) {
                Some(message) => vec![message],
                None => {
                    if let Some(entry) = self.entries.get_mut(&instance) {
                        entry.pending_watch = Some(request_id);
                    }
                    Vec::new()
                }
            },
            DelegatedClientMessage::Unwatch {
                instance,
                request_id,
                ..
            } => vec![Message::Editor(EditorMessage::Acknowledged {
                instance,
                request_id,
            })],
            DelegatedClientMessage::Operate {
                instance,
                request_id,
                block_id,
                operation_id,
                sequence,
                operation,
            } => {
                if !operate(&operation) {
                    return Vec::new();
                }
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return Vec::new();
                };
                entry.sequence = sequence;
                let mut messages = vec![Message::Client(DelegatedClientMessage::Acknowledge {
                    instance,
                    request_id,
                    block_id,
                    operation_id,
                    sequence,
                })];
                messages.extend(self.fan_out(instance, block_id, operation_id, &operation));
                messages
            }
            _ => Vec::new(),
        }
    }

    fn fan_out(
        &mut self,
        author: EditorInstanceId,
        block_id: [u8; 16],
        operation_id: [u8; 16],
        operation: &[u8],
    ) -> Vec<Message> {
        let mut instances: Vec<_> = self.entries.keys().copied().collect();
        instances.sort_by_key(|instance| instance.0);
        let mut messages = Vec::new();
        for instance in instances {
            if instance == author {
                continue;
            }
            let entry = self.entries.get_mut(&instance).unwrap();
            if entry.block.id().into_bytes() != block_id {
                continue;
            }
            entry.sequence += 1;
            messages.push(Message::Client(DelegatedClientMessage::RemoteOperation {
                instance,
                block_id,
                operation_id,
                sequence: entry.sequence,
                operation: operation.to_vec(),
            }));
        }
        messages
    }

    fn snapshot(&mut self, instance: EditorInstanceId, request_id: u64) -> Option<Message> {
        let entry = self.entries.get_mut(&instance)?;
        let data = entry.client.block_state_bytes(entry.block.id())?;
        entry.sequence = entry.block.revision();
        Some(Message::Client(DelegatedClientMessage::Snapshot {
            instance,
            request_id,
            block_id: entry.block.id().into_bytes(),
            author: entry
                .block
                .author()
                .unwrap_or(entry.client.account_id())
                .into_bytes(),
            sequence: entry.sequence,
            access: access_byte(entry.client.block_access(entry.block.id())),
            data,
        }))
    }
}

fn access_byte(access: block::BlockAccess) -> u8 {
    match access {
        block::BlockAccess::None => 0,
        block::BlockAccess::KnowExists => 1,
        block::BlockAccess::View => 2,
        block::BlockAccess::Edit => 3,
    }
}
