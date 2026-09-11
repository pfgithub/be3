use super::*;
use crate::reactive::{view, NodeRef, VisibilityBuilder};

#[test]
fn focus_loss_and_hidden_content_cancel_keyboard_activation() {
    let clicks = Rc::new(Cell::new(0));
    let counter = clicks.clone();
    let button = NodeRef::new();
    let (document, [hidden, _after]) = toolbar_of({
        let button = button.clone();
        move || {
            [
                view! {
                    <visibility visible={true}>
                        <labelled_button
                            node_ref={&button}
                            label={"Click".to_string()}
                            on_click={move || counter.set(counter.get() + 1)}
                        />
                    </visibility>
                },
                view! { <labelled_button label={"After".to_string()} /> },
            ]
        }
    });
    let button = button.get();
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, true, Modifiers::NONE)]);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(vec![key_event(Key::Space, false, Modifiers::NONE)]);
    assert_eq!(clicks.get(), 0);
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.frame(vec![key_event(Key::Enter, true, Modifiers::NONE)]);
    harness.document.set_visible(hidden, false);
    harness.frame(vec![key_event(Key::Enter, false, Modifiers::NONE)]);
    assert_eq!(clicks.get(), 0);
    assert!(!unstyled::button_focused(harness.document(), button).get());
    assert!(!unstyled::button_active(harness.document(), button).get());
}
