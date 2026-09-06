use beui::{pos2, vec2, Event, Key, Modifiers, PointerButton, Pos2};

const LINE_HEIGHT: f32 = 40.0;
const PAGE_HEIGHT: f32 = 400.0;

pub(crate) fn events(input: &egui::InputState, keyboard: bool) -> Vec<Event> {
    input
        .events
        .iter()
        .filter_map(|event| translate(event, keyboard))
        .collect()
}

fn translate(event: &egui::Event, keyboard: bool) -> Option<Event> {
    match event {
        egui::Event::PointerMoved(pos) => Some(Event::PointerMoved(position(*pos))),
        egui::Event::PointerGone => Some(Event::PointerGone),
        egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers,
        } => Some(Event::PointerButton {
            pos: position(*pos),
            button: pointer_button(*button)?,
            pressed: *pressed,
            modifiers: translate_modifiers(*modifiers),
        }),
        egui::Event::MouseWheel {
            unit,
            delta,
            modifiers,
            ..
        } if !modifiers.ctrl && !modifiers.command => {
            let scale = match unit {
                egui::MouseWheelUnit::Point => 1.0,
                egui::MouseWheelUnit::Line => LINE_HEIGHT,
                egui::MouseWheelUnit::Page => PAGE_HEIGHT,
            };
            Some(Event::Scroll(vec2(delta.x * scale, delta.y * scale)))
        }
        egui::Event::Key {
            key,
            pressed,
            repeat,
            modifiers,
            ..
        } if keyboard => Some(Event::Key {
            key: self::key(*key)?,
            pressed: *pressed,
            repeat: *repeat,
            modifiers: translate_modifiers(*modifiers),
        }),
        egui::Event::Text(text) if keyboard => Some(Event::Text(text.clone())),
        egui::Event::Paste(text) if keyboard => Some(Event::Text(text.clone())),
        _ => None,
    }
}

fn position(pos: egui::Pos2) -> Pos2 {
    pos2(pos.x, pos.y)
}

fn translate_modifiers(modifiers: egui::Modifiers) -> Modifiers {
    Modifiers {
        alt: modifiers.alt,
        ctrl: modifiers.ctrl || modifiers.command,
        shift: modifiers.shift,
    }
}

fn pointer_button(button: egui::PointerButton) -> Option<PointerButton> {
    match button {
        egui::PointerButton::Primary => Some(PointerButton::Primary),
        egui::PointerButton::Secondary => Some(PointerButton::Secondary),
        egui::PointerButton::Middle => Some(PointerButton::Middle),
        _ => None,
    }
}

fn key(key: egui::Key) -> Option<Key> {
    let key = match key {
        egui::Key::ArrowDown => Key::ArrowDown,
        egui::Key::ArrowLeft => Key::ArrowLeft,
        egui::Key::ArrowRight => Key::ArrowRight,
        egui::Key::ArrowUp => Key::ArrowUp,
        egui::Key::Backspace => Key::Backspace,
        egui::Key::Delete => Key::Delete,
        egui::Key::End => Key::End,
        egui::Key::Enter => Key::Enter,
        egui::Key::Escape => Key::Escape,
        egui::Key::Home => Key::Home,
        egui::Key::PageDown => Key::PageDown,
        egui::Key::PageUp => Key::PageUp,
        egui::Key::Space => Key::Space,
        egui::Key::Tab => Key::Tab,
        egui::Key::A => Key::A,
        egui::Key::B => Key::B,
        egui::Key::C => Key::C,
        egui::Key::D => Key::D,
        egui::Key::E => Key::E,
        egui::Key::F => Key::F,
        egui::Key::G => Key::G,
        egui::Key::H => Key::H,
        egui::Key::I => Key::I,
        egui::Key::J => Key::J,
        egui::Key::K => Key::K,
        egui::Key::L => Key::L,
        egui::Key::M => Key::M,
        egui::Key::N => Key::N,
        egui::Key::O => Key::O,
        egui::Key::P => Key::P,
        egui::Key::Q => Key::Q,
        egui::Key::R => Key::R,
        egui::Key::S => Key::S,
        egui::Key::T => Key::T,
        egui::Key::U => Key::U,
        egui::Key::V => Key::V,
        egui::Key::W => Key::W,
        egui::Key::X => Key::X,
        egui::Key::Y => Key::Y,
        egui::Key::Z => Key::Z,
        _ => return None,
    };
    Some(key)
}

pub(crate) fn cursor(icon: beui::CursorIcon) -> egui::CursorIcon {
    match icon {
        beui::CursorIcon::Default => egui::CursorIcon::Default,
        beui::CursorIcon::Crosshair => egui::CursorIcon::Crosshair,
        beui::CursorIcon::Grab => egui::CursorIcon::Grab,
        beui::CursorIcon::Grabbing => egui::CursorIcon::Grabbing,
        beui::CursorIcon::NotAllowed => egui::CursorIcon::NotAllowed,
        beui::CursorIcon::PointingHand => egui::CursorIcon::PointingHand,
        beui::CursorIcon::ResizeHorizontal => egui::CursorIcon::ResizeHorizontal,
        beui::CursorIcon::ResizeVertical => egui::CursorIcon::ResizeVertical,
        beui::CursorIcon::Text => egui::CursorIcon::Text,
        beui::CursorIcon::Wait => egui::CursorIcon::Wait,
    }
}
