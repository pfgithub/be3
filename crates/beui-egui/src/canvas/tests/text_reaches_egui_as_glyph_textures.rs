use super::*;

#[test]
fn text_reaches_egui_as_glyph_textures() {
    let mut harness = harness();
    harness.run();

    let glyphs = harness.state().canvas.glyphs.len();

    assert!(glyphs >= "Count".len());
}
