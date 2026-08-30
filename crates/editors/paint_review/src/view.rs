use block_editor_plugin::egui::{self, emath::GuiRounding as _};

use crate::render::Rendered;

const GAP: f32 = 12.0;

pub struct Panel {
    pub label: String,
    pub rendered: Rendered,
}

pub fn show(ui: &egui::Ui, region: egui::Rect, view: egui::Rect, panels: &[Panel]) -> f32 {
    let content = laid_out(panels);
    let scale = (view.width() / content.x)
        .min(view.height() / content.y)
        .max(f32::EPSILON);
    let painter = ui.painter_at(region);
    let origin = view.center() - content * scale / 2.0;
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
            let position = egui::pos2(rect.center().x, region.top() + 2.0);
            chip(
                ui,
                &painter,
                position,
                egui::Align2::CENTER_TOP,
                &panel.label,
            );
        }
        x += size.x + GAP * scale;
    }
    scale
}

pub fn caption(ui: &egui::Ui, region: egui::Rect, lines: &[String]) {
    let painter = ui.painter_at(region);
    let mut bottom = region.bottom() - 4.0;
    for line in lines.iter().rev() {
        let position = egui::pos2(region.left() + 4.0, bottom);
        bottom -= chip(ui, &painter, position, egui::Align2::LEFT_BOTTOM, line).height() + 2.0;
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

fn chip(
    ui: &egui::Ui,
    painter: &egui::Painter,
    position: egui::Pos2,
    align: egui::Align2,
    text: &str,
) -> egui::Rect {
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        egui::TextStyle::Small.resolve(ui.style()),
        ui.visuals().weak_text_color(),
    );
    let rect = align.anchor_size(position, galley.size());
    painter.rect_filled(rect.expand(2.0), 3.0, ui.visuals().extreme_bg_color);
    painter.galley(rect.min, galley, ui.visuals().weak_text_color());
    rect
}
