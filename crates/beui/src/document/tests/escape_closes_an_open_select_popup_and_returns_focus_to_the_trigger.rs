use super::*;

#[test]
fn escape_closes_an_open_select_popup_and_returns_focus_to_the_trigger() {
    let mut document = Document::new();
    let options: Vec<String> = ["Apple", "Banana"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let select = styled::select(&mut document, &options, Some(0));
    toolbar(&mut document, &[select]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let inner = harness.document().shadow_root(select);
    let trigger = unstyled::select_trigger(harness.document(), inner);
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    assert!(styled::select_open(harness.document(), select));

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(!styled::select_open(harness.document(), select));
    assert_eq!(styled::select_selected(harness.document(), select), Some(0));
    assert!(unstyled::button_focused(harness.document(), trigger));
}
