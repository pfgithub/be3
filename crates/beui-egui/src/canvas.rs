use beui::{Context, RawInput, Rect};

use crate::paint::Glyphs;
use crate::{fonts, input, paint};

pub struct Canvas {
    context: Context,
    glyphs: Glyphs,
    pixels_per_point: f32,
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            context: Context::with_fonts(&fonts::bundled()),
            glyphs: Glyphs::new(),
            pixels_per_point: 1.0,
        }
    }

    pub fn context(&self) -> &Context {
        &self.context
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        content: impl FnOnce(&Context, Rect),
    ) -> egui::Response {
        let bounds = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(bounds, egui::Sense::click_and_drag());
        self.focus(ui, &response);

        let pixels_per_point = ui.ctx().pixels_per_point();
        if pixels_per_point != self.pixels_per_point {
            self.pixels_per_point = pixels_per_point;
            self.glyphs.clear();
        }
        self.context.set_pixels_per_point(pixels_per_point);

        let keyboard = response.has_focus();
        let events = ui.input(|input| input::events(input, keyboard));
        let rect = Rect::from_min_max(
            beui::pos2(bounds.min.x, bounds.min.y),
            beui::pos2(bounds.max.x, bounds.max.y),
        );
        let output = self
            .context
            .run(RawInput { events }, |context| content(context, rect));

        paint::paint(ui, &mut self.glyphs, &output, pixels_per_point);
        if response.hovered() {
            ui.ctx().set_cursor_icon(input::cursor(output.cursor_icon));
        }
        if output.repaint {
            ui.ctx().request_repaint();
        }
        response
    }

    fn focus(&self, ui: &mut egui::Ui, response: &egui::Response) {
        let filter = egui::EventFilter {
            tab: true,
            horizontal_arrows: true,
            vertical_arrows: true,
            escape: false,
        };
        ui.memory_mut(|memory| memory.set_focus_lock_filter(response.id, filter));
        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
