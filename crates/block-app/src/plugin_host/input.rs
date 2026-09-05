use block_plugin_api::{
    DroppedFile, ImeInput, InputBatch, InputEvent, Message, Modifiers, PhysicalKey, PointerButton,
    ScreenId, ViewportMetrics, WheelUnit,
};
use eframe::egui;
use uuid::Uuid;

use super::instances::Holes;
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
        block_id: payload.block_id,
        block_type: payload.block_type,
        dropped,
    })
}

pub(super) struct FileDropEvent {
    pub(super) position: egui::Vec2,
    pub(super) files: Vec<DroppedFile>,
    pub(super) dropped: bool,
}

pub(super) fn file_drop(response: &egui::Response) -> Option<FileDropEvent> {
    let (hovering, dropped) = response.ctx.input(|input| {
        (
            !input.raw.hovered_files.is_empty(),
            input.raw.dropped_files.clone(),
        )
    });
    let pointer = response
        .ctx
        .pointer_latest_pos()
        .filter(|position| response.rect.contains(*position))?;
    if !dropped.is_empty() {
        return Some(FileDropEvent {
            position: pointer - response.rect.min,
            files: dropped.into_iter().filter_map(read_dropped).collect(),
            dropped: true,
        });
    }
    hovering.then(|| FileDropEvent {
        position: pointer - response.rect.min,
        files: Vec::new(),
        dropped: false,
    })
}

fn read_dropped(file: egui::DroppedFile) -> Option<DroppedFile> {
    let name = file
        .path
        .as_ref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| (!file.name.is_empty()).then(|| file.name.clone()))
        .unwrap_or_else(|| "File".to_owned());
    let data = match file.bytes {
        Some(bytes) => bytes.to_vec(),
        None => std::fs::read(file.path.as_ref()?).ok()?,
    };
    Some(DroppedFile { name, data })
}

#[derive(Default)]
pub(super) struct InputAdapter {
    captured: bool,
    pressed_buttons: u8,
    focused: bool,
    modifiers: Modifiers,
    over_hole: bool,
    paste_shortcut_down: bool,
}

impl InputAdapter {
    pub(super) fn update(
        &mut self,
        context: &egui::Context,
        rect: egui::Rect,
        hovered: bool,
        focused: bool,
        screen: ScreenId,
        holes: &Holes,
    ) -> Vec<Message> {
        self.over_hole = !self.captured
            && context
                .pointer_latest_pos()
                .is_some_and(|position| holes.contains(position));
        let events = context.input(|input| input.events.clone());
        let mut normalized = Vec::new();
        if focused != self.focused {
            normalized.push(InputEvent::Focus(focused));
            self.focused = focused;
        }

        if rect.width() > 0.0 && rect.height() > 0.0 {
            for event in events {
                self.normalize_event(event, rect, hovered, focused, holes, &mut normalized);
            }
        }

        let shortcut_down = super::clipboard::paste_shortcut_down();
        let shortcut_pressed = shortcut_down && !self.paste_shortcut_down;
        self.paste_shortcut_down = shortcut_down;
        if focused && shortcut_pressed {
            normalized.push(InputEvent::Paste(String::new()));
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
        holes: &Holes,
        output: &mut Vec<InputEvent>,
    ) {
        let pointer = |position: egui::Pos2, captured: bool| {
            (rect.contains(position) && !holes.contains(position)) || captured
        };
        match event {
            egui::Event::PointerMoved(position) if pointer(position, self.captured) => {
                let position = position - rect.min;
                output.push(InputEvent::PointerMoved {
                    x: position.x,
                    y: position.y,
                });
            }
            egui::Event::MouseMoved(delta) if focused => {
                output.push(InputEvent::PointerMotion {
                    x: delta.x,
                    y: delta.y,
                });
            }
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            } if pointer(pos, self.captured) => {
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
            } if hovered && !self.over_hole => {
                push_modifiers(&mut self.modifiers, modifiers, output);
                output.push(InputEvent::Wheel {
                    x: delta.x,
                    y: delta.y,
                    unit: wheel_unit(unit),
                });
            }
            egui::Event::Zoom(factor) if hovered && !self.over_hole => {
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
            egui::Event::Text(text) if focused => {
                output.push(InputEvent::Text(text));
            }
            egui::Event::Paste(text) if focused => {
                output.push(InputEvent::Paste(text));
            }
            egui::Event::Ime(ime) if focused => {
                output.push(InputEvent::Ime(match ime {
                    egui::ImeEvent::Enabled => ImeInput::Enabled,
                    egui::ImeEvent::Preedit(text) => ImeInput::Preedit(text),
                    egui::ImeEvent::Commit(text) => ImeInput::Commit(text),
                    egui::ImeEvent::Disabled => ImeInput::Disabled,
                }));
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

pub(super) fn viewport_metrics(
    size: egui::Vec2,
    visible: egui::Rect,
    scale_factor: f32,
) -> ViewportMetrics {
    let logical_width = size.x.max(0.0);
    let logical_height = size.y.max(0.0);
    let visible = visible.intersect(egui::Rect::from_min_size(egui::Pos2::ZERO, size));
    ViewportMetrics {
        logical_width,
        logical_height,
        visible_x: visible.min.x.max(0.0),
        visible_y: visible.min.y.max(0.0),
        pixel_width: (visible.width().max(0.0) * scale_factor).round() as u32,
        pixel_height: (visible.height().max(0.0) * scale_factor).round() as u32,
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
