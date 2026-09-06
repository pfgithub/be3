use std::collections::HashMap;

use beui::{FrameOutput, Galley, Glyph, GlyphId, Pos2, Rect, Shape};

pub(crate) type Glyphs = HashMap<GlyphId, egui::TextureHandle>;

pub(crate) fn paint(
    ui: &egui::Ui,
    glyphs: &mut Glyphs,
    output: &FrameOutput,
    pixels_per_point: f32,
) {
    for shape in output.shapes() {
        match shape {
            Shape::Rect {
                rect,
                corner_radius,
                stroke_width,
                color,
                clip,
            } => {
                let painter = ui.painter().with_clip_rect(clipped(ui, *clip));
                let bounds = rectangle(*rect);
                let radius =
                    egui::CornerRadius::same(corner_radius.round().clamp(0.0, 255.0) as u8);
                if *stroke_width > 0.0 {
                    painter.rect_stroke(
                        bounds,
                        radius,
                        egui::Stroke::new(*stroke_width, tint(*color)),
                        egui::StrokeKind::Inside,
                    );
                } else {
                    painter.rect_filled(bounds, radius, tint(*color));
                }
            }
            Shape::Text {
                origin,
                galley,
                color,
                clip,
            } => {
                let painter = ui.painter().with_clip_rect(clipped(ui, *clip));
                text(
                    ui,
                    &painter,
                    glyphs,
                    *origin,
                    galley,
                    tint(*color),
                    pixels_per_point,
                );
            }
        }
    }
}

fn text(
    ui: &egui::Ui,
    painter: &egui::Painter,
    glyphs: &mut Glyphs,
    origin: Pos2,
    galley: &Galley,
    color: egui::Color32,
    pixels_per_point: f32,
) {
    let full = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
    let origin = egui::vec2(
        (origin.x * pixels_per_point).round(),
        (origin.y * pixels_per_point).round(),
    );
    for glyph in galley.glyphs() {
        let texture = texture(ui, glyphs, glyph);
        let min = origin + egui::vec2(glyph.offset.x, glyph.offset.y);
        let size = egui::vec2(glyph.image.width as f32, glyph.image.height as f32);
        let bounds = egui::Rect::from_min_size(
            egui::pos2(min.x, min.y) / pixels_per_point,
            size / pixels_per_point,
        );
        painter.image(texture, bounds, full, color);
    }
}

fn texture(ui: &egui::Ui, glyphs: &mut Glyphs, glyph: &Glyph) -> egui::TextureId {
    let handle = glyphs.entry(glyph.id).or_insert_with(|| {
        let size = [glyph.image.width as usize, glyph.image.height as usize];
        let mut rgba = Vec::with_capacity(size[0] * size[1] * 4);
        for coverage in &glyph.image.pixels {
            rgba.extend_from_slice(&[255, 255, 255, *coverage]);
        }
        let image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
        ui.ctx()
            .load_texture("beui glyph", image, egui::TextureOptions::LINEAR)
    });
    handle.id()
}

fn clipped(ui: &egui::Ui, clip: Rect) -> egui::Rect {
    ui.clip_rect().intersect(rectangle(clip))
}

fn rectangle(rect: Rect) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(rect.min.x, rect.min.y),
        egui::pos2(rect.max.x, rect.max.y),
    )
}

fn tint(color: beui::Color32) -> egui::Color32 {
    let [red, green, blue, alpha] = color.to_array();
    egui::Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}
