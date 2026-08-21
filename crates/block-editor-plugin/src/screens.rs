use block_plugin_api::{
    BlockTypeDescriptor, EditorInstanceId, EditorMessage, EditorRegion, Message, ScreenId,
    ScreenLayout, ScreenRequest,
};
use block_ui::{BlockCatalog, BlockTypeEntry};
use eframe::egui;
use std::{collections::HashMap, rc::Rc};
use uuid::Uuid;

use crate::{egui_session::EguiSession, host::BlockDrag};

pub(crate) struct Screens {
    sessions: HashMap<EditorInstanceId, EguiSession>,
    open: fn(EditorInstanceId) -> EguiSession,
    requests: Vec<ScreenRequest>,
    layout: ScreenLayout,
    block_types: Rc<BlockCatalog>,
}

impl Screens {
    pub(crate) fn new<A: crate::App>() -> Self {
        Self {
            sessions: HashMap::new(),
            open: EguiSession::new::<A>,
            requests: Vec::new(),
            layout: ScreenLayout::default(),
            block_types: Rc::new(BlockCatalog::default()),
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
                session.set_block_types(Rc::clone(&self.block_types));
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
            Message::BlockTypes(descriptors) => {
                self.block_types = Rc::new(catalog(descriptors));
                for session in self.sessions.values() {
                    session.set_block_types(Rc::clone(&self.block_types));
                }
            }
            Message::Editor(EditorMessage::DragOver {
                instance,
                region,
                x,
                y,
                block_id,
                block_type,
                dropped,
            }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.set_drag(Some((
                        *region,
                        BlockDrag {
                            position: egui::pos2(*x, *y),
                            block_id: Uuid::from_bytes(*block_id),
                            block_type: Uuid::from_bytes(*block_type),
                            dropped: *dropped,
                        },
                    )));
                }
            }
            Message::Editor(EditorMessage::FilePicked {
                instance,
                request_id,
                pick,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.file_picked(*request_id, pick.clone());
                }
            }
            Message::Editor(EditorMessage::DragLeft { instance }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.set_drag(None);
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

fn catalog(descriptors: &[BlockTypeDescriptor]) -> BlockCatalog {
    BlockCatalog::new(descriptors.iter().map(|descriptor| {
        let codepoint: &'static str = Box::leak(descriptor.icon_codepoint.clone().into_boxed_str());
        (
            Uuid::from_bytes(descriptor.block_type),
            BlockTypeEntry {
                display_name: descriptor.display_name.clone(),
                icon: (!codepoint.is_empty())
                    .then(|| egui_material_icons::MaterialIcon::new(codepoint)),
            },
        )
    }))
}
