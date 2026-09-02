use block_client::{BlockClient, TunnelCarrier};
use block_plugin_api::{
    BlockTypeDescriptor, ChildStatus, EditorBand, EditorInstanceId, EditorMessage, EditorRegion,
    Message, ScreenId, ScreenLayout, ScreenRequest, TunnelMessage,
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
    chrome: Rc<Vec<EditorBand>>,
    open: fn(Rc<Vec<EditorBand>>, EditorInstanceId, Waker) -> EguiSession,
    waker: Waker,
    requests: Vec<ScreenRequest>,
    layout: ScreenLayout,
    block_types: Rc<BlockCatalog>,
    client: Option<Client>,
    theme: egui::Theme,
}

impl Screens {
    pub(crate) fn new<A: crate::App>(chrome: Vec<EditorBand>, waker: Waker) -> Self {
        Self {
            sessions: HashMap::new(),
            chrome: Rc::new(chrome),
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

    pub(crate) fn receive(&mut self, message: &Message) -> bool {
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
                client_id,
                editable,
                ..
            }) => {
                let client = self.client(
                    Uuid::from_bytes(*account_id),
                    Uuid::from_bytes(*workspace_id),
                );
                let session = self.sessions.entry(*instance).or_insert_with(|| {
                    (self.open)(Rc::clone(&self.chrome), *instance, self.waker.clone())
                });
                session.set_block_types(Rc::clone(&self.block_types));
                session.set_client_id(Uuid::from_bytes(*client_id));
                session.set_editable(*editable);
                session.connect(client, Uuid::from_bytes(*block_id));
            }
            Message::Editor(EditorMessage::OpenCreation {
                instance,
                account_id,
                workspace_id,
                client_id,
            }) => {
                let client = self.client(
                    Uuid::from_bytes(*account_id),
                    Uuid::from_bytes(*workspace_id),
                );
                let session = self.sessions.entry(*instance).or_insert_with(|| {
                    (self.open)(Rc::clone(&self.chrome), *instance, self.waker.clone())
                });
                session.set_block_types(Rc::clone(&self.block_types));
                session.set_client_id(Uuid::from_bytes(*client_id));
                session.connect_creation(client);
            }
            Message::Editor(EditorMessage::OpenArtifact {
                instance,
                block_id,
                block_type,
                account_id,
                workspace_id,
                client_id,
                data,
            }) => {
                let client = self.client(
                    Uuid::from_bytes(*account_id),
                    Uuid::from_bytes(*workspace_id),
                );
                let session = self.sessions.entry(*instance).or_insert_with(|| {
                    (self.open)(Rc::clone(&self.chrome), *instance, self.waker.clone())
                });
                session.set_block_types(Rc::clone(&self.block_types));
                session.set_client_id(Uuid::from_bytes(*client_id));
                session.connect_artifact(
                    client,
                    Uuid::from_bytes(*block_id),
                    Uuid::from_bytes(*block_type),
                    data.clone(),
                );
            }
            Message::Editor(EditorMessage::Resized {
                instance,
                width,
                height,
            }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.resized(egui::vec2(*width, *height));
                }
            }
            Message::Editor(EditorMessage::ImagePasted {
                instance,
                request_id,
                image,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_pasted_image(*request_id, image.clone());
                }
            }
            Message::Editor(EditorMessage::AudioStatus { instance, status }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_audio(status.clone());
                }
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
            Message::Editor(EditorMessage::PresentingChanged {
                instance,
                presenting,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.set_presenting(*presenting);
                }
            }
            Message::Editor(EditorMessage::Presence {
                instance,
                visible,
                entries,
            }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.presence_visible(*visible, entries.clone());
                }
            }
            Message::Editor(EditorMessage::RevealPresence {
                instance,
                client_id,
            }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.reveal_presence(*client_id);
                }
            }
            Message::Editor(EditorMessage::ReplaceChild {
                instance,
                request_id,
                old,
                new,
            }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.replace_child(
                        *request_id,
                        Uuid::from_bytes(*old),
                        Uuid::from_bytes(*new),
                    );
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
                    return false;
                };
                let Some(session) = self.sessions.get_mut(&instance) else {
                    return false;
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
                    return false;
                };
                client.carrier.send(payload.clone());
                return false;
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
            Message::Editor(EditorMessage::BlockPicked {
                instance,
                request_id,
                pick,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.block_picked(*request_id, pick.clone());
                }
            }
            Message::Editor(EditorMessage::Fetched {
                instance,
                request_id,
                result,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.fetched(*request_id, result.clone());
                }
            }
            Message::Editor(EditorMessage::AssetRead {
                instance,
                request_id,
                result,
            }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.asset_read(*request_id, result.clone());
                }
            }
            Message::Editor(EditorMessage::WebViewEvent { instance, event }) => {
                if let Some(session) = self.sessions.get(instance) {
                    session.web_view_event(event.clone());
                }
            }
            Message::ChildStatuses(statuses) => {
                let mut grouped: HashMap<EditorInstanceId, Vec<ChildStatus>> = HashMap::new();
                for status in statuses {
                    grouped
                        .entry(status.instance)
                        .or_default()
                        .push(status.clone());
                }
                for (instance, statuses) in grouped {
                    if let Some(session) = self.sessions.get(&instance) {
                        session.set_child_statuses(statuses);
                    }
                }
            }
            Message::Editor(EditorMessage::DragLeft { instance }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.set_drag(None);
                }
            }
            Message::Editor(EditorMessage::FileDrop {
                instance,
                region,
                x,
                y,
                files,
                dropped,
            }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.set_files(Some((
                        *region,
                        crate::host::FileDrop {
                            position: egui::pos2(*x, *y),
                            files: files
                                .iter()
                                .map(|file| crate::PickedFile {
                                    name: file.name.clone(),
                                    data: file.data.clone(),
                                })
                                .collect(),
                            dropped: *dropped,
                        },
                    )));
                }
            }
            Message::Editor(EditorMessage::FileDropLeft { instance }) => {
                if let Some(session) = self.sessions.get_mut(instance) {
                    session.set_files(None);
                }
            }
            _ => return false,
        }
        true
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
        self.layout = ScreenLayout::packed(&self.requests);
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
