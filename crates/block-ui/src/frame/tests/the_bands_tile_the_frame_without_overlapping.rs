use super::*;

#[test]
fn the_bands_tile_the_frame_without_overlapping() {
    let size = egui::vec2(1200.0, 800.0);
    let outcome = show(size, frame, Vec::new());
    let rects = outcome.rects;
    assert_eq!(
        rects.frame,
        egui::Rect::from_min_size(egui::Pos2::ZERO, size)
    );
    let toolbar = rects.toolbar.expect("the toolbar band is shown");
    let left = rects.left_sidebar.expect("the left sidebar is shown");
    let right = rects.right_sidebar.expect("the right sidebar is shown");
    for band in rects.bands() {
        assert_eq!(band, band.intersect(rects.frame));
    }
    assert!(toolbar.height() > 0.0);
    assert_eq!(toolbar.width(), size.x);
    assert_eq!(left.top(), toolbar.bottom());
    assert_eq!(right.top(), toolbar.bottom());
    assert_eq!(left.left(), rects.frame.left());
    assert_eq!(right.right(), rects.frame.right());
    assert_eq!(rects.content.top(), toolbar.bottom());
    assert_eq!(rects.content.left(), left.right());
    assert_eq!(rects.content.right(), right.left());
    assert_eq!(rects.content.bottom(), rects.frame.bottom());
    let bands: Vec<_> = rects.bands().collect();
    for (index, band) in bands.iter().enumerate() {
        for other in &bands[index + 1..] {
            assert!(!band.intersect(*other).is_positive());
        }
    }
}
