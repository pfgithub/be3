use crate::geometry::Rect;
use crate::painter::Shape;

pub(crate) struct Damage {
    region: Rect,
    everything: bool,
}

impl Default for Damage {
    fn default() -> Self {
        Self {
            region: Rect::NOTHING,
            everything: false,
        }
    }
}

impl Damage {
    pub(crate) fn add(&mut self, rect: Rect) {
        if rect.is_positive() {
            self.region = self.region.union(rect);
        }
    }

    pub(crate) fn everything(&mut self) {
        self.everything = true;
    }

    pub(crate) fn take(&mut self, viewport: Rect) -> Rect {
        let region = match self.everything {
            true => viewport,
            false => self.region.intersect(viewport),
        };
        *self = Self::default();
        region
    }
}

pub(crate) fn bounds(shape: &Shape) -> Rect {
    match shape {
        Shape::Rect {
            rect,
            stroke_width,
            clip,
            ..
        } => rect.expand(*stroke_width).intersect(*clip),
        Shape::Text {
            origin,
            galley,
            clip,
            ..
        } => Rect::from_min_size(*origin, galley.size()).intersect(*clip),
    }
}

#[cfg(test)]
mod tests;
