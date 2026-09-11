use super::*;
use crate::reactive::view;

#[test]
fn key_repeats_and_shortcut_modifiers_do_not_accidentally_activate_controls() {
    let clicks = Rc::new(Cell::new(0));
    let counter = clicks.clone();
    let (document, [_button]) = toolbar_of(|| {
        [view! {
            <labelled_button
                label={"Click".to_string()}
                on_click={move || counter.set(counter.get() + 1)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::CTRL);
    harness.key(Key::Space, Modifiers::ALT);
    harness.frame(vec![Event::Key {
        key: Key::Space,
        pressed: true,
        repeat: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(vec![key_event(Key::Space, false, Modifiers::NONE)]);
    assert_eq!(clicks.get(), 0);
    harness.frame(vec![key_event(Key::Space, true, Modifiers::NONE)]);
    for _ in 0..3 {
        harness.frame(vec![Event::Key {
            key: Key::Space,
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        }]);
    }
    harness.frame(vec![key_event(Key::Space, false, Modifiers::NONE)]);
    assert_eq!(clicks.get(), 1);
}
