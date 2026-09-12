use super::*;
use crate::reactive::view;
use crate::styled::SelectBuilder;

#[test]
fn enter_confirms_the_highlighted_select_option_and_closes_the_popup() {
    let options: Vec<String> = ["Apple", "Banana"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let (document, [select]) = toolbar_of(|| {
        [
            view! { <select options selected=None on_change={move |selected| {
                sink.borrow_mut().push(selected);
            }} /> },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let inner = harness.document().shadow_root(select);
    let trigger = unstyled::select_trigger(harness.document(), inner);
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(styled::select_selected(harness.document(), select), Some(0));
    assert!(!styled::select_open(harness.document(), select));
    assert_eq!(changes.borrow().as_slice(), &[Some(0)]);
    assert!(unstyled::button_focused(harness.document(), trigger).get());
}
