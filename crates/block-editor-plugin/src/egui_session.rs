use block::Block;
use block_client::{DelegatedEvent, DelegatedHostEndpoint, DelegatedRequest};
use block_plugin_api::{
    DelegatedClientMessage, EditorInstanceId, EditorMessage, InputEvent, Message, PointerButton,
    ViewportMetrics, WheelUnit,
};
use eframe::egui;
use uuid::Uuid;

pub(crate) struct EguiSession {
    app: Box<dyn AppUi>,
    input: egui::RawInput,
    metrics: Option<ViewportMetrics>,
    endpoint: Option<DelegatedHostEndpoint>,
    instance: Option<EditorInstanceId>,
    request_id: u64,
}

trait AppUi {
    fn connect(&mut self, client: block_client::BlockClient, block_id: Uuid);
    fn ui(&mut self, ui: &mut egui::Ui);
}

impl<A: crate::App> AppUi for A {
    fn connect(&mut self, client: block_client::BlockClient, block_id: Uuid) {
        crate::App::connect(self, client, block_id);
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        crate::App::ui(self, ui);
    }
}

impl EguiSession {
    pub(crate) fn new<A: crate::App>() -> Self {
        Self {
            app: Box::new(A::default()),
            input: egui::RawInput::default(),
            metrics: None,
            endpoint: None,
            instance: None,
            request_id: 1,
        }
    }
}

impl EguiSession {
    pub(crate) fn receive(&mut self, message: &Message) {
        match message {
            Message::CreateViewport(viewport) => self.metrics = Some(viewport.metrics.clone()),
            Message::ResizeViewport(metrics) => self.metrics = Some(metrics.clone()),
            Message::Input(batch) => {
                for event in &batch.events {
                    self.input(event);
                }
            }
            Message::Editor(EditorMessage::Open {
                instance,
                block_id,
                account_id,
                workspace_id,
                metrics,
                ..
            }) if self.endpoint.is_none() => {
                let (endpoint, host) = block_client::delegated_channel();
                let client = block_client::BlockClient::delegated(
                    Uuid::from_bytes(*account_id),
                    Uuid::from_bytes(*workspace_id),
                    endpoint,
                );
                self.app.connect(client, Uuid::from_bytes(*block_id));
                self.metrics = Some(metrics.clone());
                self.endpoint = Some(host);
                self.instance = Some(*instance);
            }
            Message::Editor(EditorMessage::Resize { metrics, .. }) => {
                self.metrics = Some(metrics.clone());
            }
            Message::Editor(EditorMessage::Input { batch, .. }) => {
                for event in &batch.events {
                    self.input(event);
                }
            }
            Message::Client(message) => self.client_message(message),
            _ => {}
        }
    }

    pub(crate) fn outbound(&mut self) -> Vec<Message> {
        let Some(instance) = self.instance else {
            return Vec::new();
        };
        let Some(endpoint) = &mut self.endpoint else {
            return Vec::new();
        };
        let mut messages = Vec::new();
        while let Ok(request) = endpoint.requests.try_recv() {
            let request_id = self.request_id;
            self.request_id += 1;
            let message = match request {
                DelegatedRequest::Watch { id, block_type } => DelegatedClientMessage::Watch {
                    instance,
                    request_id,
                    block_id: id.into_bytes(),
                    block_type: block_type.into_bytes(),
                },
                DelegatedRequest::Unwatch { id } => DelegatedClientMessage::Unwatch {
                    instance,
                    request_id,
                    block_id: id.into_bytes(),
                },
                DelegatedRequest::Operate {
                    id,
                    operation_id,
                    sequence,
                    operation,
                } => DelegatedClientMessage::Operate {
                    instance,
                    request_id,
                    block_id: id.into_bytes(),
                    operation_id: operation_id.into_bytes(),
                    sequence,
                    operation,
                },
            };
            messages.push(Message::Client(message));
        }
        messages
    }

    fn client_message(&mut self, message: &DelegatedClientMessage) {
        let Some(endpoint) = &mut self.endpoint else {
            return;
        };
        let event = match message {
            DelegatedClientMessage::Snapshot {
                block_id,
                author,
                sequence,
                access,
                data,
                ..
            } => DelegatedEvent::Snapshot {
                id: Uuid::from_bytes(*block_id),
                block_type: block_client::blocks::counter::Counter::TYPE_ID,
                author: Uuid::from_bytes(*author),
                sequence: *sequence,
                access: decode_access(*access),
                data: data.clone(),
            },
            DelegatedClientMessage::Acknowledge {
                block_id,
                operation_id,
                sequence,
                ..
            } => DelegatedEvent::Acknowledged {
                id: Uuid::from_bytes(*block_id),
                operation_id: Uuid::from_bytes(*operation_id),
                sequence: *sequence,
            },
            DelegatedClientMessage::RemoteOperation {
                block_id,
                operation_id,
                sequence,
                operation,
                ..
            } => DelegatedEvent::RemoteOperation {
                id: Uuid::from_bytes(*block_id),
                operation_id: Uuid::from_bytes(*operation_id),
                author: Uuid::nil(),
                sequence: *sequence,
                operation: operation.clone(),
            },
            DelegatedClientMessage::AccessChanged {
                block_id, access, ..
            } => DelegatedEvent::AccessChanged {
                id: Uuid::from_bytes(*block_id),
                access: decode_access(*access),
            },
            DelegatedClientMessage::Error { message, .. } => DelegatedEvent::Error(message.clone()),
            DelegatedClientMessage::Disconnected { message, .. } => {
                DelegatedEvent::Disconnected(message.clone())
            }
            _ => return,
        };
        let _ = endpoint.events.unbounded_send(event);
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn run(&mut self, context: &egui::Context, time: f64) -> egui::FullOutput {
        if let Some(metrics) = &self.metrics {
            context.set_pixels_per_point(metrics.scale_factor);
            self.input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(metrics.logical_width, metrics.logical_height),
            ));
        }
        self.input.time = Some(time);
        let input = std::mem::take(&mut self.input);
        self.input.focused = input.focused;
        self.input.modifiers = input.modifiers;
        context.run_ui(input, |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| self.app.ui(ui));
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn show(&mut self, ui: &mut egui::Ui) {
        self.app.ui(ui);
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn append_input(&mut self, input: &mut egui::RawInput) {
        input.events.append(&mut self.input.events);
        input.modifiers = self.input.modifiers;
        input.focused = self.input.focused;
    }

    fn input(&mut self, event: &InputEvent) {
        match event {
            InputEvent::PointerMoved { x, y } => self
                .input
                .events
                .push(egui::Event::PointerMoved(egui::pos2(*x, *y))),
            InputEvent::PointerButton {
                button,
                pressed,
                x,
                y,
            } => self.input.events.push(egui::Event::PointerButton {
                pos: egui::pos2(*x, *y),
                button: pointer_button(*button),
                pressed: *pressed,
                modifiers: self.input.modifiers,
            }),
            InputEvent::Wheel { x, y, unit } => {
                self.input.events.push(egui::Event::MouseWheel {
                    unit: wheel_unit(*unit),
                    delta: egui::vec2(*x, *y),
                    phase: egui::TouchPhase::Move,
                    modifiers: self.input.modifiers,
                });
            }
            InputEvent::Key {
                logical,
                pressed,
                repeat,
                ..
            } => {
                if let Some(key) = egui::Key::from_name(logical) {
                    self.input.events.push(egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: *pressed,
                        repeat: *repeat,
                        modifiers: self.input.modifiers,
                    });
                }
            }
            InputEvent::Text(text) => self.input.events.push(egui::Event::Text(text.clone())),
            InputEvent::Modifiers(modifiers) => {
                self.input.modifiers = egui::Modifiers {
                    alt: modifiers.alt,
                    ctrl: modifiers.control,
                    shift: modifiers.shift,
                    mac_cmd: false,
                    command: modifiers.command,
                };
            }
            InputEvent::Focus(focused) => self.input.focused = *focused,
        }
    }
}

fn decode_access(access: u8) -> block::BlockAccess {
    match access {
        3 => block::BlockAccess::Edit,
        2 => block::BlockAccess::View,
        1 => block::BlockAccess::KnowExists,
        _ => block::BlockAccess::None,
    }
}

fn pointer_button(button: PointerButton) -> egui::PointerButton {
    match button {
        PointerButton::Primary => egui::PointerButton::Primary,
        PointerButton::Secondary => egui::PointerButton::Secondary,
        PointerButton::Middle => egui::PointerButton::Middle,
        PointerButton::Back => egui::PointerButton::Extra1,
        PointerButton::Forward | PointerButton::Other(_) => egui::PointerButton::Extra2,
    }
}

fn wheel_unit(unit: WheelUnit) -> egui::MouseWheelUnit {
    match unit {
        WheelUnit::Pixels => egui::MouseWheelUnit::Point,
        WheelUnit::Lines => egui::MouseWheelUnit::Line,
        WheelUnit::Pages => egui::MouseWheelUnit::Page,
    }
}
