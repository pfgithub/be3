use block_plugin_api::{
    InputBatch, InputEvent, Message, Modifiers, PhysicalKey, PointerButton, ScreenId,
    ViewportMetrics, WheelUnit,
};
use eframe::egui;
use uuid::Uuid;

use crate::editors::SidebarDragPayload;

pub(super) struct BlockDragEvent {
    pub(super) position: egui::Vec2,
    pub(super) block_id: Uuid,
    pub(super) block_type: Uuid,
    pub(super) dropped: bool,
}

pub(super) fn block_drag(response: &egui::Response) -> Option<BlockDragEvent> {
    let (payload, dropped) = match response.dnd_release_payload::<SidebarDragPayload>() {
        Some(payload) => (payload, true),
        None => (response.dnd_hover_payload::<SidebarDragPayload>()?, false),
    };
    let pointer = response
        .ctx
        .pointer_interact_pos()
        .unwrap_or_else(|| response.rect.center());
    Some(BlockDragEvent {
        position: pointer - response.rect.min,
        block_id: payload.reference.id,
        block_type: payload.reference.block_type,
        dropped,
    })
}

#[derive(Default)]
pub(super) struct InputAdapter {
    captured: bool,
    pressed_buttons: u8,
    focused: bool,
    modifiers: Modifiers,
}

impl InputAdapter {
    pub(super) fn update(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        screen: ScreenId,
    ) -> Vec<Message> {
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

        if response.rect.width() > 0.0 && response.rect.height() > 0.0 {
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

        if normalized.is_empty() {
            return Vec::new();
        }
        vec![Message::Input(InputBatch {
            screen,
            events: normalized,
        })]
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
                #[cfg(target_os = "windows")]
                eprintln!(
                    "plugin input host pointer button={button:?} pressed={pressed} window=({:.1},{:.1}) local=({:.1},{:.1}) viewport=({:.1},{:.1})",
                    pos.x,
                    pos.y,
                    position.x,
                    position.y,
                    rect.width(),
                    rect.height()
                );
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
            egui::Event::Zoom(factor) if hovered => {
                output.push(InputEvent::Zoom { factor });
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

pub(super) fn viewport_metrics(size: egui::Vec2, scale_factor: f32) -> ViewportMetrics {
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
