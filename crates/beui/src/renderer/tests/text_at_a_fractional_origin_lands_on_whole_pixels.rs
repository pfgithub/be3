use super::*;

#[test]
fn text_at_a_fractional_origin_lands_on_whole_pixels() {
    let paint = |origin| {
        move |painter: &Painter| {
            painter.text(origin, "beui", FontId::proportional(24.0), Color32::WHITE)
        }
    };
    let aligned = capture(Color32::BLACK, paint(pos2(4.0, 4.0)));
    let nudged = capture(Color32::BLACK, paint(pos2(4.2, 4.3)));

    for y in 0..SIZE {
        for x in 0..SIZE {
            assert_eq!(
                aligned.pixel(x, y),
                nudged.pixel(x, y),
                "the glyphs moved at {x},{y}"
            );
        }
    }
}
