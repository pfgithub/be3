use super::*;

mod damage_is_the_union_of_the_rectangles_it_was_given;
mod damaging_everything_covers_the_whole_viewport;
mod taking_the_damage_clips_it_and_starts_again;
mod the_bounds_of_a_shape_stop_at_its_clip_rectangle;

use crate::color::Color32;
use crate::geometry::pos2;

fn rect(left: f32, top: f32, right: f32, bottom: f32) -> Rect {
    Rect::from_min_max(pos2(left, top), pos2(right, bottom))
}

fn clipped(bounds: Rect, clip: Rect) -> Shape {
    Shape::Rect {
        rect: bounds,
        corner_radius: 0.0,
        stroke_width: 0.0,
        color: Color32::WHITE,
        clip,
    }
}
