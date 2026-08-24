mod bitmap_ignores_pixels_outside_bounds;
mod canvas_starts_below_toolbar;
mod eraser_clears_bitmap_pixels;
mod eraser_waits_for_press;
mod pen_rasterizes_line_into_bitmap;
mod stroke_tracks_pointer_outside_canvas;
mod tool_buttons_map_to_tools;

pub(super) fn test_atlas() -> Vec<u8> {
    vec![0; (ATLAS_SIZE * ATLAS_SIZE) as usize]
}

pub(super) fn atlas(pixels: &mut [u8]) -> AtlasPixels<'_> {
    AtlasPixels::new(pixels, ATLAS_SIZE as usize)
}
use crate::renderer::{AtlasPixels, ATLAS_SIZE};
