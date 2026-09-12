use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use crate::color::Color32;
use crate::font::{FontId, FontSources, Fonts, Galley};
use crate::geometry::{pos2, vec2, Rect};
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
    test_ids: RefCell<HashMap<String, Rect>>,
    copied_text: RefCell<Option<String>>,
    cursor_icon: Cell<CursorIcon>,
    touch_emulation: Cell<bool>,
    repaint: Cell<bool>,
    repaint_after: Cell<Duration>,
    previous: RefCell<Option<(Vec<Shape>, f32)>>,
}

pub struct FrameOutput {
    pub(crate) shapes: Vec<Shape>,
    test_ids: HashMap<String, Rect>,
    pub cursor_icon: CursorIcon,
    pub copied_text: Option<String>,
    pub repaint: bool,
    pub repaint_after: Duration,
    pub changed: bool,
}

impl FrameOutput {
    pub fn shapes(&self) -> &[Shape] {
        &self.shapes
    }

    pub fn test_id_rect(&self, test_id: &str) -> Option<Rect> {
        self.test_ids.get(test_id).copied()
    }
}

impl Context {
    pub fn new() -> Self {
        Self::with_fonts(&FontSources::installed())
    }

    pub fn with_fonts(sources: &FontSources) -> Self {
        Self {
            inner: Rc::new(Inner {
                fonts: RefCell::new(Fonts::new(sources)),
                input: RefCell::new(InputState::default()),
                shapes: RefCell::new(Vec::new()),
                test_ids: RefCell::new(HashMap::new()),
                copied_text: RefCell::new(None),
                cursor_icon: Cell::new(CursorIcon::Default),
                touch_emulation: Cell::new(false),
                repaint: Cell::new(false),
                repaint_after: Cell::new(Duration::MAX),
                previous: RefCell::new(None),
            }),
        }
    }

    pub fn begin_frame(&self, raw: RawInput) {
        self.inner.input.borrow_mut().begin_frame(raw);
        self.inner.shapes.borrow_mut().clear();
        self.inner.test_ids.borrow_mut().clear();
        self.inner.copied_text.borrow_mut().take();
        self.inner.cursor_icon.set(CursorIcon::Default);
        self.inner.repaint.set(false);
        self.inner.repaint_after.set(Duration::MAX);
    }

    pub fn end_frame(&self) -> FrameOutput {
        let shapes = std::mem::take(&mut *self.inner.shapes.borrow_mut());
        let scale = self.pixels_per_point();
        let mut previous = self.inner.previous.borrow_mut();
        let changed = previous
            .as_ref()
            .is_none_or(|(old, old_scale)| *old_scale != scale || *old != shapes);
        if changed {
            *previous = Some((shapes.clone(), scale));
        }
        FrameOutput {
            shapes,
            test_ids: std::mem::take(&mut *self.inner.test_ids.borrow_mut()),
            copied_text: self.inner.copied_text.borrow_mut().take(),
            changed,
            repaint_after: self.inner.repaint_after.get(),
            cursor_icon: self.inner.cursor_icon.get(),
            repaint: self.inner.repaint.get(),
        }
    }

    pub fn run(&self, raw: RawInput, frame: impl FnOnce(&Self)) -> FrameOutput {
        self.begin_frame(raw);
        frame(self);
        self.paint_touch_cursor();
        self.end_frame()
    }

    pub fn input<R>(&self, reader: impl FnOnce(&InputState) -> R) -> R {
        reader(&self.inner.input.borrow())
    }

    pub fn painter(&self) -> Painter {
        Painter::new(self.clone(), Rect::EVERYTHING)
    }

    pub fn copy_text(&self, text: String) {
        *self.inner.copied_text.borrow_mut() = Some(text);
    }

    pub fn set_cursor_icon(&self, cursor_icon: CursorIcon) {
        self.inner.cursor_icon.set(cursor_icon);
    }

    pub(crate) fn touch_emulation(&self) -> bool {
        self.inner.touch_emulation.get()
    }

    pub(crate) fn set_touch_emulation(&self, enabled: bool) {
        self.inner.touch_emulation.set(enabled);
    }

    pub fn request_repaint(&self) {
        self.inner.repaint.set(true);
        self.request_repaint_after(Duration::ZERO);
    }

    pub fn request_repaint_after(&self, delay: Duration) {
        self.inner
            .repaint_after
            .set(self.inner.repaint_after.get().min(delay));
    }

    pub(crate) fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }

    pub(crate) fn capture(&self, paint: impl FnOnce()) -> (Vec<Shape>, Duration) {
        let previous_delay = self.inner.repaint_after.replace(Duration::MAX);
        let start = self.inner.shapes.borrow().len();
        paint();
        let delay = self.inner.repaint_after.get();
        self.request_repaint_after(previous_delay);
        (self.inner.shapes.borrow_mut().split_off(start), delay)
    }

    pub(crate) fn extend(&self, shapes: &[Shape]) {
        self.inner.shapes.borrow_mut().extend_from_slice(shapes);
    }

    pub(crate) fn publish_test_id(&self, test_id: &str, rect: Rect) {
        self.inner
            .test_ids
            .borrow_mut()
            .insert(test_id.to_owned(), rect);
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

    fn paint_touch_cursor(&self) {
        if !self.touch_emulation() {
            return;
        }
        let Some(pos) = self.input(|input| input.pointer.interact_pos()) else {
            return;
        };
        let radius = 10.0;
        let rect = Rect::from_min_size(
            pos2(pos.x - radius, pos.y - radius),
            vec2(radius * 2.0, radius * 2.0),
        );
        let painter = self.painter();
        painter.rect_filled(
            rect,
            radius,
            Color32::from_rgba_unmultiplied(255, 255, 255, 96),
        );
        painter.rect_stroke(rect, radius, 1.0, Color32::BLACK);
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
