use block_client::blocks::pixel_art::{PixelArt, PixelColor};
use block_editor_plugin::egui::{self, Color32, Rect, Sense, Stroke, Vec2};

pub fn checkerboard_image(art: &PixelArt, dark_mode: bool) -> egui::ColorImage {
    let (light, dark) = checkerboard_colors(dark_mode);
    let width = usize::from(art.width());
    let height = usize::from(art.height());
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let background = if (x + y) % 2 == 0 { light } else { dark };
            let offset = (y * width + x) * 4;
            let rgba = &art.rgba_bytes()[offset..offset + 4];
            pixels.push(composite_pixel(
                PixelColor::new(rgba[0], rgba[1], rgba[2], rgba[3]),
                background,
            ));
        }
    }
    egui::ColorImage::new([width, height], pixels)
}

pub fn checkerboard_colors(dark_mode: bool) -> ([u8; 3], [u8; 3]) {
    if dark_mode {
        ([82, 82, 82], [58, 58, 58])
    } else {
        ([232, 232, 232], [202, 202, 202])
    }
}

pub fn composite_pixel(color: PixelColor, background: [u8; 3]) -> Color32 {
    let alpha = u16::from(color.alpha);
    let inverse = 255 - alpha;
    Color32::from_rgb(
        ((u16::from(color.red) * alpha + u16::from(background[0]) * inverse) / 255) as u8,
        ((u16::from(color.green) * alpha + u16::from(background[1]) * inverse) / 255) as u8,
        ((u16::from(color.blue) * alpha + u16::from(background[2]) * inverse) / 255) as u8,
    )
}

pub fn format_hex_color(color: PixelColor) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.red, color.green, color.blue, color.alpha
    )
}

pub fn parse_hex_color(value: &str) -> Option<PixelColor> {
    let value = value.strip_prefix('#')?;
    if value.len() != 8 || !value.is_ascii() {
        return None;
    }
    Some(PixelColor::new(
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
        u8::from_str_radix(&value[6..8], 16).ok()?,
    ))
}

pub fn color_swatch(ui: &mut egui::Ui, color: PixelColor, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let half = rect.size() * 0.5;
        for (offset, background) in [
            (Vec2::ZERO, Color32::from_gray(220)),
            (Vec2::new(half.x, 0.0), Color32::from_gray(170)),
            (Vec2::new(0.0, half.y), Color32::from_gray(170)),
            (half, Color32::from_gray(220)),
        ] {
            painter.rect_filled(
                Rect::from_min_size(rect.min + offset, half),
                0.0,
                background,
            );
        }
        painter.rect_filled(
            rect,
            0.0,
            Color32::from_rgba_unmultiplied(color.red, color.green, color.blue, color.alpha),
        );
        painter.rect_stroke(
            rect,
            2.0,
            Stroke::new(
                if selected { 2.0_f32 } else { 1.0_f32 },
                if selected {
                    ui.visuals().selection.stroke.color
                } else {
                    ui.visuals().widgets.noninteractive.bg_stroke.color
                },
            ),
            egui::StrokeKind::Inside,
        );
    }
    response.on_hover_text(format_hex_color(color))
}
