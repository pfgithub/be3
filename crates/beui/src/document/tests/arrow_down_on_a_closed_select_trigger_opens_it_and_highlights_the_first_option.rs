use super::*;

#[test]
fn arrow_down_on_a_closed_select_trigger_opens_it_and_highlights_the_first_option() {
    let mut document = Document::new();
    let options: Vec<String> = ["Apple", "Banana", "Cherry"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let select = styled::select(&mut document, &options, None);
    toolbar(&mut document, &[select]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let inner = harness.document().shadow_root(select);
    let trigger = unstyled::select_trigger(harness.document(), inner);
    unstyled::focus_button(harness.document_mut(), trigger);
    harness.frame(Vec::new());

    assert!(!unstyled::select_open(harness.document(), inner));

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(unstyled::select_open(harness.document(), inner));
    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        Some(0)
    );
}
