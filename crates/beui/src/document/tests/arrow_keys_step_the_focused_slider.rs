use super::*;

#[test]
fn arrow_keys_step_the_focused_slider() {
    let mut document = Document::new();
    let slider = styled::slider(&mut document, 0.5);
    toolbar(&mut document, &[slider]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);

    let raised = styled::slider_value(harness.document(), slider);
    assert!(raised > 0.5, "the slider read {raised}");

    harness.key(Key::ArrowLeft, Modifiers::NONE);
    harness.key(Key::ArrowLeft, Modifiers::NONE);

    let lowered = styled::slider_value(harness.document(), slider);
    assert!(lowered < 0.5, "the slider read {lowered}");
}
