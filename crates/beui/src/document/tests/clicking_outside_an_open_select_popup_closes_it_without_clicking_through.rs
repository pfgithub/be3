use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::SelectBuilder;

#[test]
fn clicking_outside_an_open_select_popup_closes_it_without_clicking_through() {
    let mut document = Document::new();
    let options: Vec<String> = ["Apple", "Banana"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let select = with_reactive_scope(&mut document, || {
        view! { <select options={options} selected={Some(0)} /> }
    });
    let (other, other_clicks) = counting_button(&mut document, "Other");
    let row = document.create_list(Direction::Horizontal, 20.0);
    document.append_child(row, select, ItemSize::Intrinsic);
    document.append_child(row, other, ItemSize::Intrinsic);
    document.set_root(row);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let inner = harness.document().shadow_root(select);
    let trigger = unstyled::select_trigger(harness.document(), inner);
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    assert!(styled::select_open(harness.document(), select));

    harness.click(harness.center(other));
    harness.frame(Vec::new());

    assert!(!styled::select_open(harness.document(), select));
    assert_eq!(styled::select_selected(harness.document(), select), Some(0));
    assert_eq!(other_clicks.get(), 0);
}
