use super::{square, Canvas, CANVAS, LAND};

#[test]
fn fill_carves_holes_with_even_odd_rule() {
    let mut canvas = Canvas::new();
    let color = [10, 20, 30, 255];
    canvas.fill(&[square(100.0, 900.0), square(400.0, 600.0)], color);

    let pixel = |x: usize, y: usize| canvas.pixels[y * CANVAS + x];
    // Inside the outer ring but outside the hole.
    assert_eq!(pixel(200, 500), color);
    // Inside the hole keeps the background.
    assert_eq!(pixel(500, 500), LAND);
    // Outside the outer ring keeps the background.
    assert_eq!(pixel(50, 500), LAND);
}
