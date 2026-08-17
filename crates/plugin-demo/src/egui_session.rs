use block_plugin_api::{InputEvent, Message, PointerButton, ViewportMetrics, WheelUnit};
use eframe::egui;

#[derive(Default)]
pub(crate) struct EguiSession {
    demo: crate::demo::Demo,
    input: egui::RawInput,
    metrics: Option<ViewportMetrics>,
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
            _ => {}
        }
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
            egui::CentralPanel::default().show_inside(ui, |ui| self.demo.show(ui));
        })
    }

    pub(crate) fn show(&mut self, ui: &mut egui::Ui) {
        self.demo.show(ui);
    }

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
