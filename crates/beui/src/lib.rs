extern crate self as beui;

mod accessibility;
#[cfg(feature = "window")]
mod app;
mod base;
mod color;
mod context;
mod document;
mod font;
mod geometry;
pub mod icons;
mod input;
mod inspector;
mod interact;
mod layout;
mod node;
mod paint;
mod painter;
pub mod reactive;
#[cfg(feature = "render")]
mod renderer;
pub mod styled;
pub mod unstyled;

pub use accesskit;
#[cfg(feature = "window")]
pub use app::{run, App};
pub use base::{focus_within, Align, Direction, ItemSize, ScrollPosition, TextAlign};
pub use color::Color32;
pub use context::{Context, FrameOutput};
pub use document::Document;
pub use font::{
    FontFamily, FontId, FontSource, FontSources, Galley, Glyph, GlyphId, GlyphImage, ICONS_FONT,
};
pub use geometry::{pos2, vec2, Pos2, Rect, Vec2};
pub use input::{
    CursorIcon, Event, InputState, Key, KeyPress, Modifiers, PointerButton, PointerPress, RawInput,
    TouchId, TouchPhase, TouchPoint, TouchState,
};
pub use node::{ClickHandler, Handler, NodeId};
pub use painter::{Painter, Shape};
#[cfg(feature = "render")]
pub use renderer::{clear_color, Renderer};
