use super::*;

#[test]
fn a_border_paints_over_the_fill_it_wraps() {
    let interior = Color32::from_rgb(80, 80, 80);
    let mut document = Document::new();
    let fill = document.create_fill(interior, 0);
    let border = document.create_outline(Color32::WHITE, 1.0, 0, 0.0);
    document.set_outline_visible(border, true);
    document.set_outline_child(border, fill);
    document.set_root(border);

    let capture = capture(Color32::BLACK, |painter| {
        document.show(
            painter.ctx(),
            Rect::from_min_max(pos2(16.0, 16.0), pos2(48.0, 48.0)),
        );
    });

    assert_eq!(capture.pixel(16, 32), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(32, 16), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(47, 32), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(32, 47), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(32, 32), [80, 80, 80, 255]);
}
