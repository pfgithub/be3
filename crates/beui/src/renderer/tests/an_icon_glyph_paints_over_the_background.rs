use super::*;

#[test]
fn an_icon_glyph_paints_over_the_background() {
    let capture = capture(Color32::BLACK, |painter| {
        painter.text(
            pos2(4.0, 4.0),
            crate::icons::ICON_ADD,
            FontId::icons(32.0),
            Color32::WHITE,
        );
    });

    let painted = capture.brightest(Rect::from_min_max(Pos2::ZERO, pos2(64.0, 48.0)));
    assert!(
        painted > 200,
        "the icon glyph did not paint, brightest {painted}"
    );
}
