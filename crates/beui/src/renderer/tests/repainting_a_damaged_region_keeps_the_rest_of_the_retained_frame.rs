use super::*;

#[test]
fn repainting_a_damaged_region_keeps_the_rest_of_the_retained_frame() {
    let left = Rect::from_min_max(pos2(4.0, 4.0), pos2(28.0, 28.0));
    let right = Rect::from_min_max(pos2(36.0, 36.0), pos2(60.0, 60.0));
    let mut target = Target::new();
    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(left, 0.0, Color32::WHITE);
        painter.rect_filled(right, 0.0, Color32::WHITE);
    });

    target.draw(
        Color32::BLACK,
        Repaint::Region {
            region: right,
            background: Color32::BLACK,
        },
        |painter| {
            painter.rect_filled(left, 0.0, Color32::WHITE);
        },
    );
    let capture = target.read();

    assert_eq!(
        capture.pixel(16, 16),
        [255, 255, 255, 255],
        "the retained frame keeps what the damaged region does not cover"
    );
    assert_eq!(
        capture.pixel(48, 48),
        [0, 0, 0, 255],
        "the damaged region is cleared and repainted from the shapes it was given"
    );
}
