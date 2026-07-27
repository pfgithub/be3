#[path = "tests/bitmap_ignores_pixels_outside_bounds.rs"]
mod bitmap_ignores_pixels_outside_bounds;
#[path = "tests/canvas_starts_below_toolbar.rs"]
mod canvas_starts_below_toolbar;
#[path = "tests/eraser_clears_bitmap_pixels.rs"]
mod eraser_clears_bitmap_pixels;
#[path = "tests/eraser_waits_for_press.rs"]
mod eraser_waits_for_press;
#[path = "tests/pen_rasterizes_line_into_bitmap.rs"]
mod pen_rasterizes_line_into_bitmap;
#[path = "tests/stroke_tracks_pointer_outside_canvas.rs"]
mod stroke_tracks_pointer_outside_canvas;
#[path = "tests/tool_buttons_map_to_tools.rs"]
mod tool_buttons_map_to_tools;

pub(super) fn test_atlas() -> Vec<u8> {
    vec![0; (ATLAS_SIZE * ATLAS_SIZE) as usize]
}

pub(super) fn atlas(pixels: &mut [u8]) -> AtlasPixels<'_> {
    AtlasPixels::new(pixels, ATLAS_SIZE as usize)
}
use crate::renderer::{AtlasPixels, ATLAS_SIZE};
