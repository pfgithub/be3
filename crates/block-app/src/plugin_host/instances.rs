use block::Block;
use block_client::{blocks::counter::Counter, BlockClient, BlockHandle, Tunnel};
use block_plugin_api::{
    EditorInstanceId, EditorMessage, EditorRegion, Message, ScreenId, ScreenRequest, ScreenSet,
    TunnelMessage,
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
    screens: HashMap<EditorRegion, Screen>,
    opened: bool,
}

struct Screen {
    input: InputAdapter,
    request: ScreenRequest,
    last_seen: u64,
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
        client: Arc<BlockClient>,
        block: BlockHandle<Counter>,
        size: egui::Vec2,
        scale_factor: f32,
        pass: u64,
    ) -> ScreenId {
        let entry = self.entries.entry(instance).or_insert_with(|| {
            let tunnel = client.open_tunnel({
                let context = context.clone();
                move || context.request_repaint()
            });
            Instance {
                client,
                block,
                tunnel,
                screens: HashMap::new(),
                opened: false,
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
            screens.extend(regions);
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
