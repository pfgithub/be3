use super::*;

#[test]
fn clicking_the_padding_around_a_button_label_activates_it() {
    let mut document = Document::new();
    let (button, clicks) = counting_button(&mut document, "Click me");
    toolbar(&mut document, &[button]);
    let mut harness = Harness::new(document);

    harness.click(pos2(4.0, 4.0));

    assert_eq!(clicks.get(), 1);
}
