use super::*;

mod fill_carves_holes_with_even_odd_rule;
mod fill_clamps_spans_beyond_the_canvas;
mod rasterize_styles_water_over_land;

fn square(min: f32, max: f32) -> Vec<[f32; 2]> {
    vec![[min, min], [max, min], [max, max], [min, max]]
}
