use std::time::Instant;

use crate::geometry::{Pos2, Vec2};

const MULTI_CLICK_DELAY: f32 = 0.3;
const MULTI_CLICK_DISTANCE: f32 = 6.0;
const MULTI_CLICK_LIMIT: u32 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    Backspace,
    Delete,
    End,
    Enter,
    Escape,
    Home,
    PageDown,
    PageUp,
    Space,
    Tab,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Modifiers {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        alt: false,
        ctrl: false,
        shift: false,
    };

    pub const ALT: Self = Self {
        alt: true,
        ..Self::NONE
    };

    pub const CTRL: Self = Self {
        ctrl: true,
        ..Self::NONE
    };

    pub const SHIFT: Self = Self {
        shift: true,
        ..Self::NONE
    };
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeyPress {
    pub key: Key,
    pub pressed: bool,
    pub repeat: bool,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PointerPress {
    pub pos: Pos2,
    pub fraction: Vec2,
    pub clicks: u32,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Event {
    Key {
        key: Key,
        pressed: bool,
        repeat: bool,
        modifiers: Modifiers,
    },
    PointerButton {
        pos: Pos2,
        button: PointerButton,
        pressed: bool,
        modifiers: Modifiers,
    },
    PointerGone,
    PointerMoved(Pos2),
    Scroll(Vec2),
    Text(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum CursorIcon {
    #[default]
    Default,
    Crosshair,
    Grab,
    Grabbing,
    NotAllowed,
    PointingHand,
    ResizeHorizontal,
    ResizeVertical,
    Text,
    Wait,
}

#[derive(Clone, Default, Debug)]
pub struct RawInput {
    pub events: Vec<Event>,
}

#[derive(Clone, Default, Debug)]
pub struct InputState {
    pub events: Vec<Event>,
    pub pointer: Pointer,
    pub scroll_delta: Vec2,
    pub modifiers: Modifiers,
}

impl InputState {
    pub(crate) fn begin_frame(&mut self, raw: RawInput) {
        self.pointer.begin_frame();
        self.scroll_delta = Vec2::ZERO;
        for event in &raw.events {
            match event {
                Event::PointerMoved(pos) => self.pointer.pos = Some(*pos),
                Event::PointerGone => {
                    self.pointer.pos = None;
                    self.pointer.primary_down = false;
                }
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers,
                } => {
                    self.pointer.pos = Some(*pos);
                    self.pointer.primary_down = *pressed;
                    self.modifiers = *modifiers;
                    if *pressed {
                        self.pointer.primary_pressed = true;
                        self.pointer.count_click(*pos);
                    } else {
                        self.pointer.primary_released = true;
                    }
                }
                Event::Scroll(delta) => self.scroll_delta = self.scroll_delta + *delta,
                Event::Key { modifiers, .. } => self.modifiers = *modifiers,
                _ => {}
            }
        }
        self.events = raw.events;
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Pointer {
    pub pos: Option<Pos2>,
    pub primary_down: bool,
    pub primary_pressed: bool,
    pub primary_released: bool,
    clicks: u32,
    last_click: Option<(Instant, Pos2)>,
}

impl Pointer {
    fn begin_frame(&mut self) {
        self.primary_pressed = false;
        self.primary_released = false;
    }

    fn count_click(&mut self, pos: Pos2) {
        let now = Instant::now();
        let repeated = self.last_click.is_some_and(|(when, at)| {
            now.duration_since(when).as_secs_f32() <= MULTI_CLICK_DELAY
                && at.distance(pos) <= MULTI_CLICK_DISTANCE
        });
        self.clicks = if repeated {
            self.clicks % MULTI_CLICK_LIMIT + 1
        } else {
            1
        };
        self.last_click = Some((now, pos));
    }

    pub fn clicks(&self) -> u32 {
        self.clicks
    }

    pub fn interact_pos(&self) -> Option<Pos2> {
        self.pos
    }

    pub fn primary_pressed(&self) -> bool {
        self.primary_pressed
    }

    pub fn primary_released(&self) -> bool {
        self.primary_released
    }
}
