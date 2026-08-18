use block_plugin_api::{
    CreateViewport, InputBatch, InputEvent, Message, Modifiers, PhysicalKey, PointerButton,
    ViewportMetrics, WheelUnit,
};
use eframe::egui;

pub(super) struct InputAdapter {
    request_id: u64,
    metrics: Option<ViewportMetrics>,
    captured: bool,
    pressed_buttons: u8,
    focused: bool,
    modifiers: Modifiers,
}

impl Default for InputAdapter {
    fn default() -> Self {
        Self {
            request_id: 1,
            metrics: None,
            captured: false,
            pressed_buttons: 0,
            focused: false,
            modifiers: Modifiers::default(),
        }
    }
}

impl InputAdapter {
    pub(super) fn update(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        scale_factor: f32,
    ) -> Vec<Message> {
        let mut messages = Vec::new();
        let metrics = viewport_metrics(response.rect.size(), scale_factor);
        match &self.metrics {
            None => messages.push(Message::CreateViewport(CreateViewport {
                request_id: self.request_id,
                metrics: metrics.clone(),
            })),
            Some(previous) if previous != &metrics => {
                messages.push(Message::ResizeViewport(metrics.clone()));
            }
            _ => {}
        }
        self.metrics = Some(metrics.clone());

        if response.clicked() {
            response.request_focus();
        }
        let focused = response.has_focus();
        let events = ui.input(|input| input.events.clone());
        let mut normalized = Vec::new();
        if focused != self.focused {
            normalized.push(InputEvent::Focus(focused));
            self.focused = focused;
        }

        if metrics.pixel_width != 0 && metrics.pixel_height != 0 {
            for event in events {
                self.normalize_event(
                    event,
                    response.rect,
                    response.hovered(),
                    focused,
                    &mut normalized,
                );
            }
        }

        if !normalized.is_empty() {
            messages.push(Message::Input(InputBatch {
                viewport_request_id: self.request_id,
                events: normalized,
            }));
        }
        messages
    }

    #[cfg(target_os = "windows")]
    pub(super) fn frame(&self) -> Message {
        Message::Input(InputBatch {
            viewport_request_id: self.request_id,
            events: Vec::new(),
        })
    }

    fn normalize_event(
        &mut self,
        event: egui::Event,
        rect: egui::Rect,
        hovered: bool,
        focused: bool,
        output: &mut Vec<InputEvent>,
    ) {
        match event {
            egui::Event::PointerMoved(position) if rect.contains(position) || self.captured => {
                let position = position - rect.min;
                output.push(InputEvent::PointerMoved {
                    x: position.x,
                    y: position.y,
                });
            }
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            } if rect.contains(pos) || self.captured => {
                let button_mask = 1 << pointer_button_index(button);
                self.pressed_buttons = if pressed {
                    self.pressed_buttons | button_mask
                } else {
                    self.pressed_buttons & !button_mask
                };
                self.captured = self.pressed_buttons != 0;
                let position = pos - rect.min;
                push_modifiers(&mut self.modifiers, modifiers, output);
                output.push(InputEvent::PointerButton {
                    button: pointer_button(button),
                    pressed,
                    x: position.x,
                    y: position.y,
                });
            }
            egui::Event::MouseWheel {
                unit,
                delta,
                modifiers,
                ..
            } if hovered => {
                push_modifiers(&mut self.modifiers, modifiers, output);
                output.push(InputEvent::Wheel {
                    x: delta.x,
                    y: delta.y,
                    unit: wheel_unit(unit),
                });
            }
            egui::Event::Key {
                key,
                physical_key,
                pressed,
                repeat,
                modifiers,
            } if focused => {
                push_modifiers(&mut self.modifiers, modifiers, output);
                output.push(InputEvent::Key {
                    physical: physical_key
                        .map(|key| PhysicalKey::Code(key as u32))
                        .unwrap_or(PhysicalKey::Unidentified),
                    logical: key.name().to_owned(),
                    pressed,
                    repeat,
                });
            }
            egui::Event::Text(text) | egui::Event::Paste(text) if focused => {
                output.push(InputEvent::Text(text));
            }
            egui::Event::WindowFocused(window_focused) if !window_focused && self.focused => {
                self.focused = false;
                self.captured = false;
                self.pressed_buttons = 0;
                output.push(InputEvent::Focus(false));
            }
            _ => {}
        }
    }
}

fn viewport_metrics(size: egui::Vec2, scale_factor: f32) -> ViewportMetrics {
    let logical_width = size.x.max(0.0);
    let logical_height = size.y.max(0.0);
    ViewportMetrics {
        logical_width,
        logical_height,
        pixel_width: (logical_width * scale_factor).round() as u32,
        pixel_height: (logical_height * scale_factor).round() as u32,
        scale_factor,
    }
}

fn push_modifiers(
    previous: &mut Modifiers,
    modifiers: egui::Modifiers,
    output: &mut Vec<InputEvent>,
) {
    let modifiers = Modifiers {
        alt: modifiers.alt,
        control: modifiers.ctrl,
        shift: modifiers.shift,
        command: modifiers.command,
    };
    if *previous != modifiers {
        *previous = modifiers;
        output.push(InputEvent::Modifiers(modifiers));
    }
}

fn pointer_button(button: egui::PointerButton) -> PointerButton {
    match button {
        egui::PointerButton::Primary => PointerButton::Primary,
        egui::PointerButton::Secondary => PointerButton::Secondary,
        egui::PointerButton::Middle => PointerButton::Middle,
        egui::PointerButton::Extra1 => PointerButton::Back,
        egui::PointerButton::Extra2 => PointerButton::Forward,
    }
}

fn pointer_button_index(button: egui::PointerButton) -> u8 {
    match button {
        egui::PointerButton::Primary => 0,
        egui::PointerButton::Secondary => 1,
        egui::PointerButton::Middle => 2,
        egui::PointerButton::Extra1 => 3,
        egui::PointerButton::Extra2 => 4,
    }
}

fn wheel_unit(unit: egui::MouseWheelUnit) -> WheelUnit {
    match unit {
        egui::MouseWheelUnit::Point => WheelUnit::Pixels,
        egui::MouseWheelUnit::Line => WheelUnit::Lines,
        egui::MouseWheelUnit::Page => WheelUnit::Pages,
    }
}
