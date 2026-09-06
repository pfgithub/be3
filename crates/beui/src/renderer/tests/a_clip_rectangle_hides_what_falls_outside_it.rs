use super::*;

#[test]
fn a_clip_rectangle_hides_what_falls_outside_it() {
    let capture = capture(Color32::BLACK, |painter| {
        let clipped = painter.with_clip_rect(Rect::from_min_max(Pos2::ZERO, pos2(32.0, 64.0)));
        clipped.rect_filled(
            Rect::from_min_max(Pos2::ZERO, pos2(64.0, 64.0)),
            0.0,
            Color32::WHITE,
        );
    });

    assert_eq!(capture.pixel(16, 32), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(48, 32), [0, 0, 0, 255]);
}
