use super::super::*;

#[test]
fn canvas_starts_below_toolbar() {
    let canvas = canvas_rect(Vector::new(900.0, 520.0));

    assert_eq!(
        canvas.position[1],
        STATUS_BAR_HEIGHT + TOOLBAR_HEIGHT + OUTER_MARGIN
    );
    assert!(canvas.size[0] > 0.0);
    assert!(canvas.size[1] > 0.0);
}
