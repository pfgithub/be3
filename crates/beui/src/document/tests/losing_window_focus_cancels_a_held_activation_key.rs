use super::*;
use crate::reactive::view;

#[test]
fn losing_window_focus_cancels_a_held_activation_key() {
    let clicks = Rc::new(Cell::new(0));
    let counter = clicks.clone();
    let (document, [button]) = toolbar_of(|| {
        [view! {
            <LabelledButton
                label="Click"
                on_click={move || counter.set(counter.get() + 1)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, true, Modifiers::NONE)]);
    harness.frame(vec![Event::Focus(false)]);
    assert!(!unstyled::button_active(harness.document(), button).get());
    harness.frame(vec![
        Event::Focus(true),
        key_event(Key::Space, false, Modifiers::NONE),
    ]);
    assert_eq!(clicks.get(), 0);
    harness.key(Key::Space, Modifiers::NONE);
    assert_eq!(clicks.get(), 1);
}
