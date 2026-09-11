use super::*;
use crate::reactive::view;

#[test]
fn activation_requires_a_matching_release_and_escape_cancels_it() {
    let clicks = Rc::new(Cell::new(0));
    let counter = clicks.clone();
    let (document, [button]) = toolbar_of(|| {
        [view! {
            <labelled_button
                label={"Click".to_string()}
                on_click={move || counter.set(counter.get() + 1)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, true, Modifiers::NONE)]);
    harness.frame(vec![key_event(Key::Enter, false, Modifiers::NONE)]);
    assert_eq!(clicks.get(), 0);
    assert!(unstyled::button_active(harness.document(), button).get());
    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, false, Modifiers::NONE)]);
    assert_eq!(clicks.get(), 0);
    assert!(!unstyled::button_active(harness.document(), button).get());
    harness.key(Key::Space, Modifiers::NONE);
    assert_eq!(clicks.get(), 1);
}
