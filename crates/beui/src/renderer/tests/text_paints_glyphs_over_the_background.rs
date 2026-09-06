use super::*;

#[test]
fn text_paints_glyphs_over_the_background() {
    let capture = capture(Color32::BLACK, |painter| {
        painter.text(
            pos2(4.0, 4.0),
            "beui",
            FontId::proportional(24.0),
            Color32::WHITE,
        );
    });

    let painted = capture.brightest(Rect::from_min_max(Pos2::ZERO, pos2(64.0, 40.0)));
    assert!(
        painted > 200,
        "the glyphs did not paint, brightest {painted}"
    );
    assert_eq!(capture.pixel(32, 60), [0, 0, 0, 255]);
}
