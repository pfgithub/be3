use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;
use std::time::Instant;

use crate::color::Color32;
use crate::font::{FontId, Galley};
use crate::geometry::{pos2, vec2, Pos2, Rect, Vec2};
use crate::painter::Painter;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId};

const CARET_WIDTH: f32 = 1.0;
const BLINK_INTERVAL: f32 = 0.53;

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
    placeholder: String,
    font_size: f32,
    color: Color32,
    placeholder_color: Color32,
    selection_color: Color32,
    caret_color: Color32,
    horizontal: TextAlign,
    vertical: TextAlign,
    wrap: bool,
    monospace: bool,
    clip: bool,
    caret: Option<usize>,
    selection: Vec<Range<usize>>,
    blink: Instant,
    offset: Cell<f32>,
    placed: RefCell<Option<Placed>>,
}

impl TextNode {
    fn font(&self) -> FontId {
        if self.monospace {
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

    fn showing_placeholder(&self) -> bool {
        self.content.is_empty() && !self.placeholder.is_empty()
    }

    fn caret_shown(&self) -> bool {
        let phase = (self.blink.elapsed().as_secs_f32() / BLINK_INTERVAL) as u32;
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
        let content = self.galley(painter, &self.content, available.x).size();
        if self.placeholder.is_empty() {
            return content;
        }
        content.max(self.galley(painter, &self.placeholder, available.x).size())
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

        if self.showing_placeholder() {
            let galley = self.galley(&painter, &self.placeholder, rect.width());
            let origin = self.origin(galley.size(), rect);
            painter.galley(origin, galley, self.placeholder_color);
        } else {
            painter.galley(placed.origin, placed.galley.clone(), self.color);
        }

        if let Some(caret) = self.caret {
            if self.caret_shown() {
                let top = placed.galley.cursor_pos(placed.origin, caret);
                painter.rect_filled(
                    Rect::from_min_size(top, vec2(CARET_WIDTH, placed.galley.line_height())),
                    0.0,
                    self.caret_color,
                );
            }
            painter.ctx().request_repaint();
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
    pub fn create_text(
        &mut self,
        content: impl Into<String>,
        font_size: f32,
        color: Color32,
    ) -> NodeId {
        self.arena.insert(TextNode {
            content: content.into(),
            placeholder: String::new(),
            font_size,
            color,
            placeholder_color: Color32::from_gray(140),
            selection_color: Color32::from_gray(80),
            caret_color: color,
            horizontal: TextAlign::Start,
            vertical: TextAlign::Start,
            wrap: false,
            monospace: false,
            clip: false,
            caret: None,
            selection: Vec::new(),
            blink: Instant::now(),
            offset: Cell::new(0.0),
            placed: RefCell::new(None),
        })
    }

    pub fn set_text(&mut self, id: NodeId, content: impl Into<String>) {
        self.arena.get_mut_as::<TextNode>(id).content = content.into();
    }

    pub fn text(&self, id: NodeId) -> &str {
        &self.arena.get_as::<TextNode>(id).content
    }

    pub fn set_text_align(&mut self, text: NodeId, horizontal: TextAlign, vertical: TextAlign) {
        let node = self.arena.get_mut_as::<TextNode>(text);
        node.horizontal = horizontal;
        node.vertical = vertical;
    }

    pub fn set_text_wrap(&mut self, text: NodeId, wrap: bool) {
        self.arena.get_mut_as::<TextNode>(text).wrap = wrap;
    }

    pub fn set_text_font_size(&mut self, text: NodeId, font_size: f32) {
        self.arena.get_mut_as::<TextNode>(text).font_size = font_size;
    }

    pub fn set_text_monospace(&mut self, text: NodeId, monospace: bool) {
        self.arena.get_mut_as::<TextNode>(text).monospace = monospace;
    }

    pub fn set_text_color(&mut self, text: NodeId, color: Color32) {
        self.arena.get_mut_as::<TextNode>(text).color = color;
    }

    pub fn set_text_placeholder(&mut self, text: NodeId, placeholder: impl Into<String>) {
        self.arena.get_mut_as::<TextNode>(text).placeholder = placeholder.into();
    }

    pub fn set_text_placeholder_color(&mut self, text: NodeId, color: Color32) {
        self.arena.get_mut_as::<TextNode>(text).placeholder_color = color;
    }

    pub fn set_text_selection_color(&mut self, text: NodeId, color: Color32) {
        self.arena.get_mut_as::<TextNode>(text).selection_color = color;
    }

    pub fn set_text_caret_color(&mut self, text: NodeId, color: Color32) {
        self.arena.get_mut_as::<TextNode>(text).caret_color = color;
    }

    pub fn set_text_clip(&mut self, text: NodeId, clip: bool) {
        self.arena.get_mut_as::<TextNode>(text).clip = clip;
    }

    pub fn set_text_caret(&mut self, text: NodeId, caret: Option<usize>) {
        let node = self.arena.get_mut_as::<TextNode>(text);
        node.caret = caret;
        node.blink = Instant::now();
    }

    pub fn set_text_selection(&mut self, text: NodeId, selection: Vec<Range<usize>>) {
        self.arena.get_mut_as::<TextNode>(text).selection = selection;
    }

    pub fn text_index_at(&self, text: NodeId, pos: Pos2) -> usize {
        self.arena.get_as::<TextNode>(text).index_at(pos)
    }
}
