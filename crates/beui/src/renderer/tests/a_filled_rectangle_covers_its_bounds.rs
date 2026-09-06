use super::*;

#[test]
fn a_filled_rectangle_covers_its_bounds() {
    let capture = capture(Color32::BLACK, |painter| {
        painter.rect_filled(
            Rect::from_min_max(pos2(16.0, 16.0), pos2(48.0, 48.0)),
            0.0,
            Color32::WHITE,
        );
    });

    assert_eq!(capture.pixel(32, 32), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(4, 4), [0, 0, 0, 255]);
}
