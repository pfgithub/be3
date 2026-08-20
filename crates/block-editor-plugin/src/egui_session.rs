use block_client::TunnelCarrier;
use block_plugin_api::{
    EditorInstanceId, InputEvent, Message, PointerButton, ScreenPlacement, TunnelMessage, WheelUnit,
};
use eframe::egui;
use uuid::Uuid;

pub(crate) struct EguiSession {
    app: Box<dyn AppUi>,
    instance: EditorInstanceId,
    input: egui::RawInput,
    placement: Option<ScreenPlacement>,
    carrier: Option<TunnelCarrier>,
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
    pub(crate) fn new<A: crate::App>(instance: EditorInstanceId) -> Self {
        Self {
            app: Box::new(A::default()),
            instance,
            input: egui::RawInput::default(),
            placement: None,
            carrier: None,
        }
    }

    pub(crate) fn connect(&mut self, block_id: Uuid, account_id: Uuid, workspace_id: Uuid) {
        if self.carrier.is_some() {
            return;
        }
        let (endpoint, carrier) = block_client::tunnel_channel();
        let client = block_client::BlockClient::tunneled(account_id, workspace_id, endpoint);
        self.app.connect(client, block_id);
        self.carrier = Some(carrier);
    }

    pub(crate) fn place(&mut self, placement: ScreenPlacement) {
        self.placement = Some(placement);
    }

    pub(crate) fn outbound(&mut self) -> Vec<Message> {
        let instance = self.instance;
        let Some(carrier) = &mut self.carrier else {
            return Vec::new();
        };
        let mut messages = Vec::new();
        while let Some(payload) = carrier.try_recv() {
            messages.push(Message::Client(TunnelMessage::Request {
                instance,
                payload,
            }));
        }
        messages
    }

    pub(crate) fn client_message(&mut self, message: &TunnelMessage) {
        let Some(carrier) = &self.carrier else {
            return;
        };
        if let TunnelMessage::Response { payload, .. } = message {
            carrier.send(payload.clone());
        }
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn run(&mut self, context: &egui::Context, time: f64) -> egui::FullOutput {
        context.set_pixels_per_point(self.scale_factor());
        self.input.screen_rect = Some(self.rect());
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
        let rect = self.rect();
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(rect)
                .id_salt(self.instance.0),
            |ui| self.app.ui(ui),
        );
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn append_input(&mut self, input: &mut egui::RawInput) {
        input.events.append(&mut self.input.events);
        input.modifiers = self.input.modifiers;
        input.focused |= self.input.focused;
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn scale_factor(&self) -> f32 {
        self.placement
            .as_ref()
            .map_or(1.0, ScreenPlacement::scale_factor)
    }

    fn rect(&self) -> egui::Rect {
        let Some(placement) = &self.placement else {
            return egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO);
        };
        let scale = placement.scale_factor();
        egui::Rect::from_min_size(
            egui::pos2(placement.x as f32 / scale, placement.y as f32 / scale),
            egui::vec2(
                placement.width as f32 / scale,
                placement.height as f32 / scale,
            ),
        )
    }

    pub(crate) fn input(&mut self, event: &InputEvent) {
        let origin = self.rect().min.to_vec2();
        match event {
            InputEvent::PointerMoved { x, y } => self
                .input
                .events
                .push(egui::Event::PointerMoved(egui::pos2(*x, *y) + origin)),
            InputEvent::PointerButton {
                button,
                pressed,
                x,
                y,
            } => {
                self.input.events.push(egui::Event::PointerButton {
                    pos: egui::pos2(*x, *y) + origin,
                    button: pointer_button(*button),
                    pressed: *pressed,
                    modifiers: self.input.modifiers,
                });
            }
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
