use super::*;

#[test]
fn slider_home_end_and_page_keys_clamp_at_the_bounds() {
    let mut document = Document::new();
    let slider = styled::slider(&mut document, 0.5);
    toolbar(&mut document, &[slider]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::End, Modifiers::NONE);
    harness.key(Key::PageUp, Modifiers::NONE);
    assert_eq!(styled::slider_value(harness.document(), slider), 1.0);
    harness.key(Key::PageDown, Modifiers::NONE);
    assert!((styled::slider_value(harness.document(), slider) - 0.8).abs() < 0.001);
    harness.key(Key::Home, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(styled::slider_value(harness.document(), slider), 0.0);
}
