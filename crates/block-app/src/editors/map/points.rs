use block_client::{
    block_ref::BlockRef,
    blocks::map::{MapColor, MapPoint},
};
use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use uuid::Uuid;

use super::geo::MapView;
use crate::editors::name_galley;

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
    label: impl Fn(BlockRef) -> Option<(String, bool)>,
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
        if let Some((label, automatic)) = label(point.block_id) {
            draw_label(painter, tip, &label, automatic, opacity);
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

fn draw_label(painter: &Painter, tip: Pos2, label: &str, automatic: bool, opacity: f32) {
    let position = tip + Vec2::new(0.0, 3.0);
    let font = FontId::proportional(LABEL_SIZE);
    let halo = Color32::from_rgba_unmultiplied(255, 255, 255, 190).gamma_multiply(opacity);
    let text_color = Color32::from_gray(30).gamma_multiply(opacity);
    let halo_galley = name_galley(painter, label, font.clone(), halo, automatic);
    for offset in [
        Vec2::new(-1.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, -1.0),
        Vec2::new(0.0, 1.0),
    ] {
        let anchor = Align2::CENTER_TOP.anchor_size(position + offset, halo_galley.size());
        painter.galley(anchor.min, halo_galley.clone(), halo);
    }
    let text_galley = name_galley(painter, label, font, text_color, automatic);
    let anchor = Align2::CENTER_TOP.anchor_size(position, text_galley.size());
    painter.galley(anchor.min, text_galley, text_color);
}

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
