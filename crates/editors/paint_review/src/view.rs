use block_editor_plugin::egui::{self, emath::GuiRounding as _};

use crate::render::Rendered;

const GAP: f32 = 12.0;

pub fn show(ui: &egui::Ui, region: egui::Rect, view: egui::Rect, panels: &[Rendered]) -> f32 {
    let content = laid_out(panels);
    let scale = (view.width() / content.x)
        .min(view.height() / content.y)
        .max(f32::EPSILON);
    let painter = ui.painter_at(region);
    let origin = view.center() - content * scale / 2.0;
    let mut x = 0.0;
    for panel in panels {
        let size = panel.size * scale;
        let top = origin.y + (content.y * scale - size.y) / 2.0;
        let rect = egui::Rect::from_min_size(egui::pos2(origin.x + x, top), size)
            .round_to_pixels(ui.pixels_per_point());
        painter.image(
            panel.texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        x += size.x + GAP * scale;
    }
    scale
}

fn laid_out(panels: &[Rendered]) -> egui::Vec2 {
    let width: f32 = panels.iter().map(|panel| panel.size.x).sum::<f32>()
        + GAP * panels.len().saturating_sub(1) as f32;
    let height = panels.iter().map(|panel| panel.size.y).fold(1.0, f32::max);
    egui::vec2(width.max(1.0), height)
}
