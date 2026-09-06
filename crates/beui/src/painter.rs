use crate::color::Color32;
use crate::context::Context;
use crate::font::{FontId, Galley};
use crate::geometry::{Pos2, Rect};

pub(crate) enum Shape {
    Rect {
        rect: Rect,
        corner_radius: f32,
        stroke_width: f32,
        color: Color32,
        clip: Rect,
    },
    Text {
        origin: Pos2,
        galley: Galley,
        color: Color32,
        clip: Rect,
    },
}

pub struct Painter {
    context: Context,
    clip: Rect,
}

impl Painter {
    pub(crate) fn new(context: Context, clip: Rect) -> Self {
        Self { context, clip }
    }

    pub fn ctx(&self) -> &Context {
        &self.context
    }

    pub fn clip_rect(&self) -> Rect {
        self.clip
    }

    pub fn with_clip_rect(&self, clip: Rect) -> Self {
        Self {
            context: self.context.clone(),
            clip: self.clip.intersect(clip),
        }
    }

    pub fn layout(&self, text: impl Into<String>, font: FontId, wrap_width: f32) -> Galley {
        self.context.layout(&text.into(), font, wrap_width)
    }

    pub fn rect_filled(&self, rect: Rect, corner_radius: f32, color: Color32) {
        if color.alpha() == 0 {
            return;
        }
        self.context.push(Shape::Rect {
            rect,
            corner_radius,
            stroke_width: 0.0,
            color,
            clip: self.clip,
        });
    }

    pub fn rect_stroke(&self, rect: Rect, corner_radius: f32, width: f32, color: Color32) {
        if color.alpha() == 0 || width <= 0.0 {
            return;
        }
        self.context.push(Shape::Rect {
            rect,
            corner_radius,
            stroke_width: width,
            color,
            clip: self.clip,
        });
    }

    pub fn galley(&self, origin: Pos2, galley: Galley, color: Color32) {
        if color.alpha() == 0 || galley.glyphs().is_empty() {
            return;
        }
        self.context.push(Shape::Text {
            origin,
            galley,
            color,
            clip: self.clip,
        });
    }

    pub fn text(&self, origin: Pos2, text: impl Into<String>, font: FontId, color: Color32) {
        let galley = self.layout(text, font, f32::INFINITY);
        self.galley(origin, galley, color);
    }
}
