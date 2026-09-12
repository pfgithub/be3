use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;
use std::time::{Duration, Instant};

use crate::color::Color32;
use crate::font::{FontId, Galley};
use crate::geometry::{pos2, vec2, Pos2, Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};
use crate::reactive::{create_effect, with_document, NodeRef, Prop};

use beui_macros::component;

const CARET_WIDTH: f32 = 1.0;
const BLINK_INTERVAL: Duration = Duration::from_millis(530);
const DEFAULT_FONT_SIZE: f32 = 14.0;
const DEFAULT_SELECTION_COLOR: Color32 = Color32::from_gray(80);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextAlign {
    Start,
    Center,
    End,
}

#[derive(Clone)]
struct Placed {
    rect: Rect,
    galley: Galley,
    origin: Pos2,
}

pub(crate) struct TextNode {
    content: String,
    font_size: f32,
    color: Color32,
    selection_color: Color32,
    caret_color: Color32,
    horizontal: TextAlign,
    vertical: TextAlign,
    wrap: bool,
    monospace: bool,
    icon: bool,
    clip: bool,
    caret: Option<usize>,
    selection: Vec<Range<usize>>,
    blink: Instant,
    offset: Cell<f32>,
    placed: RefCell<Option<Placed>>,
}

impl TextNode {
    fn font(&self) -> FontId {
        if self.icon {
            FontId::icons(self.font_size)
        } else if self.monospace {
            FontId::monospace(self.font_size)
        } else {
            FontId::proportional(self.font_size)
        }
    }

    fn wrap_width(&self, available_width: f32) -> f32 {
        if self.wrap {
            available_width.max(0.0)
        } else {
            f32::INFINITY
        }
    }

    fn galley(&self, painter: &Painter, text: &str, available_width: f32) -> Galley {
        painter.layout(
            text.to_owned(),
            self.font(),
            self.wrap_width(available_width),
        )
    }

    fn origin(&self, size: Vec2, rect: Rect) -> Pos2 {
        let x = match self.horizontal {
            TextAlign::Start => rect.left(),
            TextAlign::Center => rect.center().x - size.x / 2.0,
            TextAlign::End => rect.right() - size.x,
        };
        let y = match self.vertical {
            TextAlign::Start => rect.top(),
            TextAlign::Center => rect.center().y - size.y / 2.0,
            TextAlign::End => rect.bottom() - size.y,
        };
        pos2(x - self.offset.get(), y)
    }

    fn scrolled(&self, galley: &Galley, rect: Rect) -> f32 {
        if !self.clip {
            return 0.0;
        }
        let mut offset = self.offset.get();
        if let Some(caret) = self.caret {
            let caret = galley.cursor_pos(Pos2::ZERO, caret).x;
            if caret < offset {
                offset = caret;
            }
            if caret > offset + rect.width() - CARET_WIDTH {
                offset = caret - rect.width() + CARET_WIDTH;
            }
        }
        offset.clamp(0.0, (galley.size().x - rect.width()).max(0.0))
    }

    fn placed(&self, painter: &Painter, rect: Rect) -> Placed {
        if let Some(placed) = self
            .placed
            .borrow()
            .as_ref()
            .filter(|placed| placed.rect == rect)
        {
            return placed.clone();
        }
        self.place(painter, rect)
    }

    fn place(&self, painter: &Painter, rect: Rect) -> Placed {
        let galley = self.galley(painter, &self.content, rect.width());
        self.offset.set(self.scrolled(&galley, rect));
        let origin = self.origin(galley.size(), rect);
        let placed = Placed {
            rect,
            galley,
            origin,
        };
        *self.placed.borrow_mut() = Some(placed.clone());
        placed
    }

    fn caret_shown(&self) -> bool {
        let phase = (self.blink.elapsed().as_secs_f64() / BLINK_INTERVAL.as_secs_f64()) as u32;
        phase.is_multiple_of(2)
    }

    fn index_at(&self, pos: Pos2) -> usize {
        match self.placed.borrow().as_ref() {
            Some(placed) => placed.galley.cursor_at(placed.origin, pos),
            None => 0,
        }
    }
}

impl Element for TextNode {
    fn measure(&self, _doc: &Document, painter: &Painter, available: Vec2) -> Vec2 {
        self.galley(painter, &self.content, available.x).size()
    }

    fn layout(
        &self,
        _doc: &Document,
        painter: &Painter,
        rect: Rect,
        _out: &mut HashMap<NodeId, Rect>,
    ) {
        self.place(painter, rect);
    }

    fn paint(
        &self,
        _doc: &Document,
        painter: &Painter,
        _rects: &HashMap<NodeId, Rect>,
        rect: Rect,
    ) {
        let painter = painter.with_clip_rect(if self.clip { rect } else { Rect::EVERYTHING });
        let placed = self.placed(&painter, rect);

        for range in &self.selection {
            for area in placed.galley.selection_rects(placed.origin, range.clone()) {
                painter.rect_filled(area, 0.0, self.selection_color);
            }
        }

        painter.galley(placed.origin, placed.galley.clone(), self.color);

        if let Some(caret) = self.caret {
            if self.caret_shown() {
                let top = placed.galley.cursor_pos(placed.origin, caret);
                painter.rect_filled(
                    Rect::from_min_size(top, vec2(CARET_WIDTH, placed.galley.line_height())),
                    0.0,
                    self.caret_color,
                );
            }
            let elapsed = self.blink.elapsed().as_nanos();
            let remaining = BLINK_INTERVAL.as_nanos() - elapsed % BLINK_INTERVAL.as_nanos();
            painter
                .ctx()
                .request_repaint_after(Duration::from_nanos(remaining as u64));
        }
    }

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
    ) -> Vec<NodeId> {
        Vec::new()
    }

    fn children(&self) -> Vec<NodeId> {
        Vec::new()
    }

    fn kind(&self) -> &'static str {
        "text"
    }

    fn detail(&self) -> Option<String> {
        Some(format!("\"{}\"", self.content))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub(crate) fn create_text(
        &mut self,
        content: impl Into<String>,
        font_size: f32,
        color: Color32,
    ) -> NodeId {
        self.arena.insert(TextNode {
            content: content.into(),
            font_size,
            color,
            selection_color: DEFAULT_SELECTION_COLOR,
            caret_color: color,
            horizontal: TextAlign::Start,
            vertical: TextAlign::Start,
            wrap: false,
            monospace: false,
            icon: false,
            clip: false,
            caret: None,
            selection: Vec::new(),
            blink: Instant::now(),
            offset: Cell::new(0.0),
            placed: RefCell::new(None),
        })
    }

    pub(crate) fn set_text(&mut self, id: NodeId, content: impl Into<String>) {
        let value = content.into();
        if self.arena.get_as::<TextNode>(id).content != value {
            self.arena.get_mut_as::<TextNode>(id).content = value;
        }
    }

    pub fn text(&self, id: NodeId) -> &str {
        &self.arena.get_as::<TextNode>(id).content
    }

    pub(crate) fn set_text_horizontal_align(&mut self, text: NodeId, horizontal: TextAlign) {
        if self.arena.get_as::<TextNode>(text).horizontal != horizontal {
            self.arena.get_mut_as::<TextNode>(text).horizontal = horizontal;
        }
    }

    pub(crate) fn set_text_vertical_align(&mut self, text: NodeId, vertical: TextAlign) {
        if self.arena.get_as::<TextNode>(text).vertical != vertical {
            self.arena.get_mut_as::<TextNode>(text).vertical = vertical;
        }
    }

    pub(crate) fn set_text_wrap(&mut self, text: NodeId, wrap: bool) {
        if self.arena.get_as::<TextNode>(text).wrap != wrap {
            self.arena.get_mut_as::<TextNode>(text).wrap = wrap;
        }
    }

    pub(crate) fn set_text_font_size(&mut self, text: NodeId, font_size: f32) {
        if self.arena.get_as::<TextNode>(text).font_size != font_size {
            self.arena.get_mut_as::<TextNode>(text).font_size = font_size;
        }
    }

    pub(crate) fn set_text_monospace(&mut self, text: NodeId, monospace: bool) {
        if self.arena.get_as::<TextNode>(text).monospace != monospace {
            self.arena.get_mut_as::<TextNode>(text).monospace = monospace;
        }
    }

    pub(crate) fn set_text_icon(&mut self, text: NodeId, icon: bool) {
        if self.arena.get_as::<TextNode>(text).icon != icon {
            self.arena.get_mut_as::<TextNode>(text).icon = icon;
        }
    }

    pub(crate) fn set_text_color(&mut self, text: NodeId, color: Color32) {
        if self.arena.get_as::<TextNode>(text).color != color {
            self.arena.get_mut_as::<TextNode>(text).color = color;
        }
    }

    pub(crate) fn set_text_selection_color(&mut self, text: NodeId, color: Color32) {
        if self.arena.get_as::<TextNode>(text).selection_color != color {
            self.arena.get_mut_as::<TextNode>(text).selection_color = color;
        }
    }

    pub(crate) fn set_text_caret_color(&mut self, text: NodeId, color: Color32) {
        if self.arena.get_as::<TextNode>(text).caret_color != color {
            self.arena.get_mut_as::<TextNode>(text).caret_color = color;
        }
    }

    pub(crate) fn set_text_clip(&mut self, text: NodeId, clip: bool) {
        if self.arena.get_as::<TextNode>(text).clip != clip {
            self.arena.get_mut_as::<TextNode>(text).clip = clip;
        }
    }

    pub(crate) fn set_text_caret(&mut self, text: NodeId, caret: Option<usize>) {
        if caret.is_none() && self.arena.get_as::<TextNode>(text).caret.is_none() {
            return;
        }
        let node = self.arena.get_mut_as::<TextNode>(text);
        node.caret = caret;
        node.blink = Instant::now();
    }

    pub(crate) fn set_text_selection(&mut self, text: NodeId, selection: Vec<Range<usize>>) {
        if self.arena.get_as::<TextNode>(text).selection != selection {
            self.arena.get_mut_as::<TextNode>(text).selection = selection;
        }
    }

    pub(crate) fn text_index_at(&self, text: NodeId, pos: Pos2) -> usize {
        self.arena.get_as::<TextNode>(text).index_at(pos)
    }
}

pub fn text_index_at(text: &NodeRef, pos: Pos2) -> usize {
    let text = text.get();
    with_document(|document| document.text_index_at(text, pos))
}

#[component(base)]
pub fn text(
    string: Prop<String>,
    #[prop(default = DEFAULT_FONT_SIZE)] font_size: Prop<f32>,
    #[prop(default = Color32::WHITE)] color: Prop<Color32>,
    #[prop(default = DEFAULT_SELECTION_COLOR)] selection_color: Prop<Color32>,
    #[prop(default = Color32::WHITE)] caret_color: Prop<Color32>,
    caret: Prop<Option<usize>>,
    selection: Prop<Vec<Range<usize>>>,
    align: Option<Prop<TextAlign>>,
    vertical_align: Option<Prop<TextAlign>>,
    wrap: Prop<bool>,
    monospace: Prop<bool>,
    icon: Prop<bool>,
    clip: Prop<bool>,
) -> NodeId {
    let vertical_default = match align {
        Some(_) => TextAlign::Center,
        None => TextAlign::Start,
    };
    let align = align.unwrap_or(Prop::Static(TextAlign::Start));
    let vertical_align = vertical_align.unwrap_or(Prop::Static(vertical_default));
    let node = with_document(|document| {
        document.create_text(String::new(), DEFAULT_FONT_SIZE, Color32::WHITE)
    });
    create_effect(move || {
        with_document(|document| document.set_text_horizontal_align(node, align.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_text_vertical_align(node, vertical_align.get()))
    });
    create_effect(move || with_document(|document| document.set_text_wrap(node, wrap.get())));
    create_effect(move || {
        with_document(|document| document.set_text_monospace(node, monospace.get()))
    });
    create_effect(move || with_document(|document| document.set_text_icon(node, icon.get())));
    create_effect(move || with_document(|document| document.set_text_clip(node, clip.get())));
    create_effect(move || with_document(|document| document.set_text(node, string.get())));
    create_effect(move || {
        with_document(|document| document.set_text_font_size(node, font_size.get()))
    });
    create_effect(move || with_document(|document| document.set_text_color(node, color.get())));
    create_effect(move || {
        with_document(|document| document.set_text_selection_color(node, selection_color.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_text_caret_color(node, caret_color.get()))
    });
    create_effect(move || with_document(|document| document.set_text_caret(node, caret.get())));
    create_effect(move || {
        with_document(|document| document.set_text_selection(node, selection.get()))
    });
    node
}
