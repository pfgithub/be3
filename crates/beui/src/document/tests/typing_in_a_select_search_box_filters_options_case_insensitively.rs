use super::*;
use crate::reactive::view;
use crate::styled::SelectBuilder;

#[test]
fn typing_in_a_select_search_box_filters_options_case_insensitively() {
    let options: Vec<String> = ["Apple", "Banana", "Grape"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let (document, [select]) =
        toolbar_of(|| [view! { <select options={options} selected={None} /> }]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let inner = harness.document().shadow_root(select);
    let trigger = unstyled::select_trigger(harness.document(), inner);
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());

    harness.type_text("BAN");
    harness.frame(Vec::new());

    let apple = unstyled::select_option_button(harness.document(), inner, 0);
    let banana = unstyled::select_option_button(harness.document(), inner, 1);
    let grape = unstyled::select_option_button(harness.document(), inner, 2);
    assert!(harness.document().node_rect(apple).is_none());
    assert!(harness.document().node_rect(banana).is_some());
    assert!(harness.document().node_rect(grape).is_none());
    assert_eq!(
        unstyled::select_highlighted(harness.document(), inner),
        Some(1)
    );
}
