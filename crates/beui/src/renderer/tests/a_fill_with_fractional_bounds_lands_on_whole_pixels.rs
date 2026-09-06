use super::*;

#[test]
fn a_fill_with_fractional_bounds_lands_on_whole_pixels() {
    let capture = capture(Color32::BLACK, |painter| {
        painter.rect_filled(
            Rect::from_min_max(pos2(16.3, 16.3), pos2(47.6, 47.6)),
            0.0,
            Color32::WHITE,
        );
    });

    assert_eq!(capture.pixel(15, 32), [0, 0, 0, 255]);
    assert_eq!(capture.pixel(16, 32), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(47, 32), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(48, 32), [0, 0, 0, 255]);
}
