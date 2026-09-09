use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::SelectBuilder;

#[test]
fn arrow_down_on_a_closed_select_trigger_opens_it_and_highlights_the_first_option() {
    let mut document = Document::new();
    let options: Vec<String> = ["Apple", "Banana", "Cherry"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let select = with_reactive_scope(&mut document, || {
        view! { <select options={options} selected={None} /> }
    });
    toolbar(&mut document, &[select]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let inner = harness.document().shadow_root(select);
    let trigger = unstyled::select_trigger(harness.document(), inner);
    with_installed(harness.document_mut(), |_document| {
        unstyled::focus_button(trigger);
    });
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
