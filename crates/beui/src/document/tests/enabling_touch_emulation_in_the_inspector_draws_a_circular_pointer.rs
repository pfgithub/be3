use super::*;

#[test]
fn enabling_touch_emulation_in_the_inspector_draws_a_circular_pointer() {
    let mut harness = Harness::new(hello_column().document);

    harness.toggle_inspector();
    assert!(!harness.touch_emulation());

    harness.click(harness.touch_toggle_center());
    assert!(harness.touch_emulation());

    let output = harness.frame(vec![]);
    assert!(output.shapes().iter().any(|shape| matches!(
        shape,
        crate::painter::Shape::Rect {
            rect,
            corner_radius: 10.0,
            ..
        } if rect.size() == Vec2::splat(20.0)
    )));
}
