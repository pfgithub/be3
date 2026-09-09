use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::SelectBuilder;

#[test]
fn opening_a_select_focuses_its_search_box_and_highlights_the_selected_option() {
    let mut document = Document::new();
    let options: Vec<String> = ["Apple", "Banana", "Cherry"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let select = with_reactive_scope(&mut document, || {
        view! { <select options={options} selected={Some(1)} /> }
    });
    toolbar(&mut document, &[select]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let inner = harness.document().shadow_root(select);
    let trigger = unstyled::select_trigger(harness.document(), inner);
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());

    assert!(styled::select_open(harness.document(), select));
    let search = unstyled::select_search(harness.document(), inner);
    assert!(unstyled::text_input_focused(harness.document(), search).get());
    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        Some(1)
    );
}
