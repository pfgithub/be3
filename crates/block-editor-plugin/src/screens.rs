use block_client::{BlockClient, TunnelCarrier};
use block_plugin_api::{
    BlockTypeDescriptor, EditorInstanceId, EditorMessage, EditorRegion, Message, ScreenId,
    ScreenLayout, ScreenRequest, TunnelMessage,
};
use block_ui::{BlockCatalog, BlockTypeEntry};
use eframe::egui;
use std::{collections::HashMap, rc::Rc, sync::Arc};
use uuid::Uuid;

use crate::{egui_session::EguiSession, host::BlockDrag, Waker};

struct Client {
    client: Arc<BlockClient>,
    carrier: TunnelCarrier,
    account_id: Uuid,
    workspace_id: Uuid,
}

pub(crate) struct Screens {
    sessions: HashMap<EditorInstanceId, EguiSession>,
    open: fn(EditorInstanceId, Waker) -> EguiSession,
    waker: Waker,
    requests: Vec<ScreenRequest>,
    layout: ScreenLayout,
    block_types: Rc<BlockCatalog>,
    client: Option<Client>,
    theme: egui::Theme,
}

impl Screens {
    pub(crate) fn new<A: crate::App>(waker: Waker) -> Self {
        Self {
            sessions: HashMap::new(),
            open: EguiSession::new::<A>,
            waker,
            requests: Vec::new(),
            layout: ScreenLayout::default(),
            block_types: Rc::new(BlockCatalog::default()),
            client: None,
            theme: egui::Theme::Dark,
        }
    }

    fn client(&mut self, account_id: Uuid, workspace_id: Uuid) -> Arc<BlockClient> {
        if let Some(existing) = &self.client {
            if existing.account_id == account_id && existing.workspace_id == workspace_id {
                return Arc::clone(&existing.client);
            }
        }
        let (endpoint, carrier) = block_client::tunnel_channel();
        let waker = self.waker.clone();
        let client = Arc::new(BlockClient::tunneled(
            account_id,
            workspace_id,
            endpoint,
            move || waker.wake(),
        ));
        self.client = Some(Client {
            client: Arc::clone(&client),
            carrier,
            account_id,
            workspace_id,
        });
        client
    }

    pub(crate) fn theme(&self) -> egui::Theme {
        self.theme
    }

    pub(crate) fn waker(&self) -> Waker {
        self.waker.clone()
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    pub(crate) fn set_generation(&mut self, generation: u64) {
        self.layout.generation = generation;
    }

    pub(crate) fn receive(&mut self, message: &Message) {
        match message {
            Message::HelloAccepted(accepted) => {
                self.theme = match accepted.dark_theme {
                    true => egui::Theme::Dark,
                    false => egui::Theme::Light,
                };
            }
            Message::Editor(EditorMessage::Open {
                instance,
                block_id,
                account_id,
                workspace_id,
                editable,
                ..
            }) => {
                let client = self.client(
                    Uuid::from_bytes(*account_id),
                    Uuid::from_bytes(*workspace_id),
                );
                let session = self
                    .sessions
                    .entry(*instance)
                    .or_insert_with(|| (self.open)(*instance, self.waker.clone()));
                session.set_block_types(Rc::clone(&self.block_types));
                session.set_editable(*editable);
                session.connect(client, Uuid::from_bytes(*block_id));
            }
            Message::Editor(EditorMessage::OpenCreation {
                instance,
                account_id,
                workspace_id,
            }) => {
                let client = self.client(
                    Uuid::from_bytes(*account_id),
                    Uuid::from_bytes(*workspace_id),
                );
                let session = self
                    .sessions
                    .entry(*instance)
                    .or_insert_with(|| (self.open)(*instance, self.waker.clone()));
                session.set_block_types(Rc::clone(&self.block_types));
                session.connect_creation(client);
            }
            Message::Editor(EditorMessage::OpenArtifact {
                instance,
                block_id,
                block_type,
                account_id,
                workspace_id,
                data,
            }) => {
                let client = self.client(
                    Uuid::from_bytes(*account_id),
                    Uuid::from_bytes(*workspace_id),
                );
                let session = self
                    .sessions
                    .entry(*instance)
                    .or_insert_with(|| (self.open)(*instance, self.waker.clone()));
                session.set_block_types(Rc::clone(&self.block_types));
                session.connect_artifact(
                    client,
                    Uuid::from_bytes(*block_id),
                    Uuid::from_bytes(*block_type),
                    data.clone(),
                );
            }
            Message::Editor(EditorMessage::ArtifactSettings { instance, data }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.artifact_settings(data.clone());
                }
            }
            Message::Editor(EditorMessage::RegenerateArtifact { instance, data }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.regenerate_artifact(data);
                }
            }
            Message::Editor(EditorMessage::CommitCreation { instance }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.commit_creation();
                }
            }
            Message::Editor(EditorMessage::EditabilityChanged { instance, editable }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_editable(*editable);
                }
            }
            Message::Editor(EditorMessage::ViewChanged {
                instance,
                x,
                y,
                width,
                height,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_view(egui::Rect::from_min_size(
                        egui::pos2(*x, *y),
                        egui::vec2(*width, *height),
                    ));
                }
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
            Message::Client(TunnelMessage::Response { payload }) => {
                let Some(client) = &self.client else {
                    log(&format!(
                        "dropped a server frame: the runtime has no client: {}",
                        summary(payload)
                    ));
                    return;
                };
                client.carrier.send(payload.clone());
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
        if let Some(client) = &mut self.client {
            while let Some(payload) = client.carrier.try_recv() {
                log(&format!("sending a client frame: {}", summary(&payload)));
                messages.push(Message::Client(TunnelMessage::Request { payload }));
            }
        }
        for session in self.sessions.values_mut() {
            messages.extend(session.outbound());
        }
        messages
    }

    pub(crate) fn session(&mut self, instance: EditorInstanceId) -> Option<&mut EguiSession> {
        self.sessions.get_mut(&instance)
    }

    pub(crate) fn is_open(&self, instance: EditorInstanceId) -> bool {
        self.sessions.contains_key(&instance)
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
            session.place(
                placements.get(instance).map_or(&[], Vec::as_slice),
                &self.requests,
            );
        }
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
    eprintln!("{message}");
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
