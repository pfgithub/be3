use super::*;

#[test]
fn arrow_keys_in_a_select_search_box_move_the_highlighted_option_without_editing_the_search_text() {
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
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    let search = unstyled::select_search(harness.document(), inner);

    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        None
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        Some(0)
    );
    assert_eq!(unstyled::text_input_value(harness.document(), search), "");

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        Some(1)
    );
    assert_eq!(unstyled::text_input_value(harness.document(), search), "");

    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        Some(0)
    );
}
