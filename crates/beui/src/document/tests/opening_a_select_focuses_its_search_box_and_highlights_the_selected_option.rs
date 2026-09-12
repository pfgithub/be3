use super::*;
use crate::reactive::view;
use crate::styled::Select;

#[test]
fn opening_a_select_focuses_its_search_box_and_highlights_the_selected_option() {
    let options: Vec<String> = ["Apple", "Banana", "Cherry"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let (document, [select]) = toolbar_of(|| [view! { <Select options selected=Some(1) /> }]);
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
