use block_editor_plugin::egui::{self, emath::GuiRounding as _};

use crate::render::Rendered;

const MINIMUM: f32 = 0.05;
const MAXIMUM: f32 = 32.0;
const GAP: f32 = 12.0;
const STEP: f32 = 1.25;
const WHEEL: f32 = 0.004;

pub struct Panel {
    pub label: String,
    pub rendered: Rendered,
}

#[derive(Default)]
pub struct Viewport {
    zoom: Option<f32>,
    offset: egui::Vec2,
    fitted: f32,
}

impl Viewport {
    pub fn fit(&mut self) {
        self.zoom = None;
        self.offset = egui::Vec2::ZERO;
    }

    pub fn fitting(&self) -> bool {
        self.zoom.is_none()
    }

    pub fn set(&mut self, scale: f32) {
        self.zoom = Some(scale.clamp(MINIMUM, MAXIMUM));
        self.offset = egui::Vec2::ZERO;
    }

    pub fn nudge(&mut self, factor: f32) {
        let scale = self.scale();
        self.zoom = Some((scale * factor).clamp(MINIMUM, MAXIMUM));
    }

    pub fn zoom_in(&mut self) {
        self.nudge(STEP);
    }

    pub fn zoom_out(&mut self) {
        self.nudge(1.0 / STEP);
    }

    pub fn scale(&self) -> f32 {
        self.zoom.unwrap_or(self.fitted)
    }

    pub fn show(&mut self, ui: &mut egui::Ui, panels: &[Panel]) {
        let available = ui.available_size().max(egui::Vec2::splat(1.0));
        let content = laid_out(panels);
        let (viewport, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
        self.fitted = (available.x / content.x)
            .min(available.y / content.y)
            .clamp(MINIMUM, 1.0);

        if response.dragged() {
            self.zoom = Some(self.scale());
            self.offset += response.drag_delta();
        }
        if response.hovered() {
            self.wheel(ui, viewport.center());
        }

        let scale = self.scale();
        let slack = ((content * scale - available) / 2.0).max(egui::Vec2::ZERO);
        self.offset = self.offset.clamp(-slack, slack);

        let painter = ui.painter_at(viewport);
        let origin = viewport.center() + self.offset - content * scale / 2.0;
        let mut x = 0.0;
        for panel in panels {
            let size = panel.rendered.size * scale;
            let top = origin.y + (content.y * scale - size.y) / 2.0;
            let rect = egui::Rect::from_min_size(egui::pos2(origin.x + x, top), size)
                .round_to_pixels(ui.pixels_per_point());
            painter.image(
                panel.rendered.texture.id(),
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            if panels.len() > 1 {
                label(ui, &painter, viewport, rect, &panel.label);
            }
            x += size.x + GAP * scale;
        }
    }

    fn wheel(&mut self, ui: &egui::Ui, center: egui::Pos2) {
        let (scroll, pinch, pointer) = ui.input(|input| {
            (
                input.smooth_scroll_delta.y,
                input.zoom_delta(),
                input.pointer.hover_pos(),
            )
        });
        let factor = (scroll * WHEEL).exp() * pinch;
        if (factor - 1.0).abs() < f32::EPSILON {
            return;
        }
        let scale = self.scale();
        let wanted = (scale * factor).clamp(MINIMUM, MAXIMUM);
        let ratio = wanted / scale;
        if let Some(pointer) = pointer {
            self.offset = (pointer - center) * (1.0 - ratio) + self.offset * ratio;
        }
        self.zoom = Some(wanted);
    }
}

fn laid_out(panels: &[Panel]) -> egui::Vec2 {
    let width: f32 = panels
        .iter()
        .map(|panel| panel.rendered.size.x)
        .sum::<f32>()
        + GAP * panels.len().saturating_sub(1) as f32;
    let height = panels
        .iter()
        .map(|panel| panel.rendered.size.y)
        .fold(1.0, f32::max);
    egui::vec2(width.max(1.0), height)
}

fn label(
    ui: &egui::Ui,
    painter: &egui::Painter,
    viewport: egui::Rect,
    rect: egui::Rect,
    text: &str,
) {
    let position = egui::pos2(rect.center().x, viewport.top() + 2.0);
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        egui::TextStyle::Small.resolve(ui.style()),
        ui.visuals().weak_text_color(),
    );
    let background = egui::Rect::from_center_size(
        egui::pos2(position.x, position.y + galley.size().y / 2.0),
        galley.size() + egui::vec2(8.0, 2.0),
    );
    painter.rect_filled(background, 3.0, ui.visuals().extreme_bg_color);
    painter.galley(
        egui::pos2(position.x - galley.size().x / 2.0, position.y),
        galley,
        ui.visuals().weak_text_color(),
    );
}
