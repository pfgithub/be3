use super::{square, Canvas, CANVAS, WATER};

#[test]
fn fill_clamps_spans_beyond_the_canvas() {
    let mut canvas = Canvas::new();
    let inside = square(10.0, 20.0);
    let left = CANVAS as f32 + 10.0;
    let right = CANVAS as f32 + 20.0;
    let beyond = vec![[left, 10.0], [right, 10.0], [right, 20.0], [left, 20.0]];
    canvas.fill(&[inside, beyond], WATER);

    assert_eq!(canvas.pixels[15 * CANVAS + 15], WATER);
}
