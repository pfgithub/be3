use super::{square, Canvas, CANVAS, LAND};

#[test]
fn fill_carves_holes_with_even_odd_rule() {
    let mut canvas = Canvas::new();
    let color = [10, 20, 30, 255];
    canvas.fill(&[square(100.0, 900.0), square(400.0, 600.0)], color);

    let pixel = |x: usize, y: usize| canvas.pixels[y * CANVAS + x];

    assert_eq!(pixel(200, 500), color);

    assert_eq!(pixel(500, 500), LAND);

    assert_eq!(pixel(50, 500), LAND);
}
