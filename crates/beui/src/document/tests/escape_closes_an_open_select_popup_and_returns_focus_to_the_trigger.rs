use super::*;
use crate::reactive::view;
use crate::styled::Select;

#[test]
fn escape_closes_an_open_select_popup_and_returns_focus_to_the_trigger() {
    let options: Vec<String> = ["Apple", "Banana"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let (document, [select]) = toolbar_of(|| [view! { <Select options selected=Some(0) /> }]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let inner = select;
    let trigger = unstyled::select_trigger(harness.document(), inner);
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    assert!(styled::select_open(harness.document(), select));

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(!styled::select_open(harness.document(), select));
    assert_eq!(styled::select_selected(harness.document(), select), Some(0));
    assert!(unstyled::button_focused(harness.document(), trigger).get());
}
