//! Drawing and hit testing of the blocks pinned to the map.

use block_client::blocks::map::{MapColor, MapPoint};
use eframe::egui::{Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use uuid::Uuid;

use super::geo::MapView;

const HEAD_RADIUS: f32 = 7.0;
const HEIGHT: f32 = 18.0;
const HIT_RADIUS: f32 = 14.0;
const LABEL_SIZE: f32 = 11.0;
const DEFAULT_COLOR: Color32 = Color32::from_rgb(224, 49, 49);

pub(super) fn marker_color(color: MapColor) -> Color32 {
    match color {
        MapColor::Default => DEFAULT_COLOR,
        MapColor::Rgb { red, green, blue } => Color32::from_rgb(red, green, blue),
    }
}

/// The screen rect a marker occupies, used to keep off-screen markers from
/// being drawn or picked.
fn marker_rect(tip: Pos2) -> Rect {
    Rect::from_min_max(
        Pos2::new(tip.x - HEAD_RADIUS, tip.y - HEIGHT - HEAD_RADIUS),
        Pos2::new(tip.x + HEAD_RADIUS, tip.y),
    )
}

pub(super) fn draw_points(
    painter: &Painter,
    view: MapView,
    clip: Rect,
    points: &[MapPoint],
    label: impl Fn(Uuid) -> Option<String>,
    selected: Option<Uuid>,
    opacity: f32,
) {
    for point in points {
        let tip = view.position(point.position);
        if !clip.expand(HEIGHT + HEAD_RADIUS).contains(tip) {
            continue;
        }
        draw_marker(
            painter,
            tip,
            marker_color(point.color),
            selected == Some(point.id),
            opacity,
        );
        if let Some(label) = label(point.block_id) {
            draw_label(painter, tip, &label, opacity);
        }
    }
}

fn draw_marker(painter: &Painter, tip: Pos2, color: Color32, selected: bool, opacity: f32) {
    let head = tip - Vec2::new(0.0, HEIGHT);
    let color = color.gamma_multiply(opacity);
    let outline = Color32::WHITE.gamma_multiply(opacity);
    painter.add(Shape::convex_polygon(
        vec![
            tip,
            head + Vec2::new(-HEAD_RADIUS * 0.75, HEAD_RADIUS * 0.5),
            head + Vec2::new(HEAD_RADIUS * 0.75, HEAD_RADIUS * 0.5),
        ],
        color,
        Stroke::NONE,
    ));
    painter.circle(head, HEAD_RADIUS, color, Stroke::new(1.5_f32, outline));
    painter.circle_filled(head, HEAD_RADIUS * 0.35, outline);
    if selected {
        painter.circle_stroke(
            head,
            HEAD_RADIUS + 3.5,
            Stroke::new(
                2.0_f32,
                Color32::from_rgb(66, 153, 225).gamma_multiply(opacity),
            ),
        );
    }
}

fn draw_label(painter: &Painter, tip: Pos2, label: &str, opacity: f32) {
    let position = tip + Vec2::new(0.0, 3.0);
    let font = FontId::proportional(LABEL_SIZE);
    let halo = Color32::from_rgba_unmultiplied(255, 255, 255, 190).gamma_multiply(opacity);
    for offset in [
        Vec2::new(-1.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, -1.0),
        Vec2::new(0.0, 1.0),
    ] {
        painter.text(
            position + offset,
            eframe::egui::Align2::CENTER_TOP,
            label,
            font.clone(),
            halo,
        );
    }
    painter.text(
        position,
        eframe::egui::Align2::CENTER_TOP,
        label,
        font,
        Color32::from_gray(30).gamma_multiply(opacity),
    );
}

/// The topmost marker under `position`, matching the order markers are drawn.
pub(super) fn point_at(points: &[MapPoint], view: MapView, position: Pos2) -> Option<Uuid> {
    points
        .iter()
        .rev()
        .find(|point| {
            let tip = view.position(point.position);
            marker_rect(tip).expand(2.0).contains(position) || tip.distance(position) <= HIT_RADIUS
        })
        .map(|point| point.id)
}
