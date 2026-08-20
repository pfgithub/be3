use block::Block;
use block_client::{blocks::counter::Counter, BlockClient, BlockHandle, Tunnel};
use block_plugin_api::{
    EditorInstanceId, EditorMessage, Message, ScreenId, ScreenRequest, ScreenSet, TunnelMessage,
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
    tunnel: Tunnel,
    input: InputAdapter,
    screen: ScreenId,
    request: ScreenRequest,
    last_seen: u64,
    opened: bool,
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
        context: &egui::Context,
        client: Arc<BlockClient>,
        block: BlockHandle<Counter>,
        size: egui::Vec2,
        scale_factor: f32,
        pass: u64,
    ) -> ScreenId {
        let next_screen = &mut self.next_screen;
        let entry = self.entries.entry(instance).or_insert_with(|| {
            *next_screen += 1;
            let tunnel = client.open_tunnel({
                let context = context.clone();
                move || context.request_repaint()
            });
            Instance {
                client,
                block,
                tunnel,
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
        }
        messages
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
