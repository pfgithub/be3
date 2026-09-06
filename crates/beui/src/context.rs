use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::font::{FontId, Fonts, Galley};
use crate::geometry::Rect;
use crate::input::{CursorIcon, InputState, RawInput};
use crate::painter::{Painter, Shape};

#[derive(Clone)]
pub struct Context {
    inner: Rc<Inner>,
}

struct Inner {
    fonts: RefCell<Fonts>,
    input: RefCell<InputState>,
    shapes: RefCell<Vec<Shape>>,
    cursor_icon: Cell<CursorIcon>,
    repaint: Cell<bool>,
}

pub struct FrameOutput {
    pub(crate) shapes: Vec<Shape>,
    pub cursor_icon: CursorIcon,
    pub repaint: bool,
}

impl Context {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(Inner {
                fonts: RefCell::new(Fonts::new()),
                input: RefCell::new(InputState::default()),
                shapes: RefCell::new(Vec::new()),
                cursor_icon: Cell::new(CursorIcon::Default),
                repaint: Cell::new(false),
            }),
        }
    }

    pub fn begin_frame(&self, raw: RawInput) {
        self.inner.input.borrow_mut().begin_frame(raw);
        self.inner.shapes.borrow_mut().clear();
        self.inner.cursor_icon.set(CursorIcon::Default);
        self.inner.repaint.set(false);
    }

    pub fn end_frame(&self) -> FrameOutput {
        FrameOutput {
            shapes: std::mem::take(&mut self.inner.shapes.borrow_mut()),
            cursor_icon: self.inner.cursor_icon.get(),
            repaint: self.inner.repaint.get(),
        }
    }

    pub fn run(&self, raw: RawInput, frame: impl FnOnce(&Self)) -> FrameOutput {
        self.begin_frame(raw);
        frame(self);
        self.end_frame()
    }

    pub fn input<R>(&self, reader: impl FnOnce(&InputState) -> R) -> R {
        reader(&self.inner.input.borrow())
    }

    pub fn painter(&self) -> Painter {
        Painter::new(self.clone(), Rect::EVERYTHING)
    }

    pub fn set_cursor_icon(&self, cursor_icon: CursorIcon) {
        self.inner.cursor_icon.set(cursor_icon);
    }

    pub fn request_repaint(&self) {
        self.inner.repaint.set(true);
    }

    pub fn pixels_per_point(&self) -> f32 {
        self.inner.fonts.borrow().pixels_per_point()
    }

    pub fn set_pixels_per_point(&self, pixels_per_point: f32) {
        self.inner
            .fonts
            .borrow_mut()
            .set_pixels_per_point(pixels_per_point);
    }

    pub(crate) fn layout(&self, text: &str, font: FontId, wrap_width: f32) -> Galley {
        self.inner.fonts.borrow_mut().layout(text, font, wrap_width)
    }

    pub(crate) fn push(&self, shape: Shape) {
        self.inner.shapes.borrow_mut().push(shape);
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
