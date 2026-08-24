mod ignores_second_touch_while_first_is_active;
mod touch_input_draws_notes_stroke;

pub(super) fn test_atlas() -> Vec<u8> {
    vec![0; (ATLAS_SIZE * ATLAS_SIZE) as usize]
}

pub(super) fn atlas(pixels: &mut [u8]) -> AtlasPixels<'_> {
    AtlasPixels::new(pixels, ATLAS_SIZE as usize)
}
mod vulkan_shaders_target_spirv_1_0;
use super::{AtlasPixels, ATLAS_SIZE};
