use super::*;
use crate::reactive::view;

#[test]
fn enter_activates_the_focused_button() {
    let clicks = Rc::new(Cell::new(0));
    let counter = clicks.clone();
    let (document, [button]) = toolbar_of(|| {
        [view! {
            <labelled_button
                label={"Click me".to_string()}
                on_click={move || counter.set(counter.get() + 1)}
            />
        }]
    });
    let active = unstyled::button_active(&document, button);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Enter, true, Modifiers::NONE)]);
    assert!(active.get());

    harness.frame(vec![key_event(Key::Enter, false, Modifiers::NONE)]);
    assert!(!active.get());
    assert_eq!(clicks.get(), 1);
}
