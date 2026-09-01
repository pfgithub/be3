use super::*;

#[test]
fn a_narrow_frame_floats_its_sidebars_and_keeps_the_content_full_width() {
    let size = egui::vec2(COMPACT_FRAME_WIDTH - 1.0, 800.0);
    let outcome = show(size, frame, Vec::new());
    let rects = outcome.rects;
    let toolbar = rects.toolbar.expect("the toolbar band is shown");
    assert_eq!(rects.content.left(), rects.frame.left());
    assert_eq!(rects.content.right(), rects.frame.right());
    assert_eq!(rects.content.top(), toolbar.bottom());
    let left = rects.left_sidebar.expect("the left sidebar floats");
    let right = rects.right_sidebar.expect("the right sidebar floats");
    assert!(left.width() > 0.0);
    assert!(right.width() > 0.0);
}
