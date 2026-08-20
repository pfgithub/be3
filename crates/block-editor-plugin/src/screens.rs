use block_plugin_api::{
    EditorInstanceId, EditorMessage, EditorRegion, Message, ScreenId, ScreenLayout, ScreenRequest,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::egui_session::EguiSession;

pub(crate) struct Screens {
    sessions: HashMap<EditorInstanceId, EguiSession>,
    open: fn(EditorInstanceId) -> EguiSession,
    requests: Vec<ScreenRequest>,
    layout: ScreenLayout,
}

impl Screens {
    pub(crate) fn new<A: crate::App>() -> Self {
        Self {
            sessions: HashMap::new(),
            open: EguiSession::new::<A>,
            requests: Vec::new(),
            layout: ScreenLayout::default(),
        }
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn set_generation(&mut self, generation: u64) {
        self.layout.generation = generation;
    }

    pub(crate) fn receive(&mut self, message: &Message) {
        match message {
            Message::Editor(EditorMessage::Open {
                instance,
                block_id,
                account_id,
                workspace_id,
                ..
            }) => {
                let session = self
                    .sessions
                    .entry(*instance)
                    .or_insert_with(|| (self.open)(*instance));
                session.connect(
                    Uuid::from_bytes(*block_id),
                    Uuid::from_bytes(*account_id),
                    Uuid::from_bytes(*workspace_id),
                );
            }
            Message::Editor(EditorMessage::Close { instance }) => {
                self.sessions.remove(instance);
                self.requests
                    .retain(|request| request.instance != *instance);
                self.relayout();
            }
            Message::Screens(set) => {
                self.requests = set.screens.clone();
                self.relayout();
            }
            Message::Input(batch) => {
                let Some((instance, region)) = self.screen(batch.screen) else {
                    return;
                };
                let Some(session) = self.sessions.get_mut(&instance) else {
                    return;
                };
                for event in &batch.events {
                    session.input(region, event);
                }
            }
            Message::Client(message) => {
                if let Some(session) = self.sessions.get_mut(&instance_of_client(message)) {
                    session.client_message(message);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn outbound(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        for session in self.sessions.values_mut() {
            messages.extend(session.outbound());
        }
        messages
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn placements(&self) -> Vec<block_plugin_api::ScreenPlacement> {
        self.layout.screens.clone()
    }

    pub(crate) fn session(&mut self, instance: EditorInstanceId) -> Option<&mut EguiSession> {
        self.sessions.get_mut(&instance)
    }

    fn screen(&self, screen: ScreenId) -> Option<(EditorInstanceId, EditorRegion)> {
        self.layout
            .placement(screen)
            .map(|placement| (placement.instance, placement.region))
    }

    fn relayout(&mut self) {
        let generation = self.layout.generation;
        self.layout = ScreenLayout::stacked(&self.requests);
        self.layout.generation = generation;
        let mut placements: HashMap<EditorInstanceId, Vec<_>> = HashMap::new();
        for placement in &self.layout.screens {
            placements
                .entry(placement.instance)
                .or_default()
                .push(*placement);
        }
        for (instance, session) in &mut self.sessions {
            session.place(placements.get(instance).map_or(&[], Vec::as_slice));
        }
    }
}

fn instance_of_client(message: &block_plugin_api::TunnelMessage) -> EditorInstanceId {
    use block_plugin_api::TunnelMessage as Tunnel;
    let (Tunnel::Request { instance, .. } | Tunnel::Response { instance, .. }) = message;
    *instance
}
