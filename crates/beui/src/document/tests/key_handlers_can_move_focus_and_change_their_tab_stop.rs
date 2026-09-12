use super::*;
use crate::reactive::{create_memo, create_signal, NodeRef};
use crate::KeyPress;

#[test]
fn key_handlers_can_move_focus_and_change_their_tab_stop() {
    let (skipped, set_skipped) = create_signal(false);
    let second = NodeRef::new();
    let (document, [_first, _second]) = toolbar_of({
        let second = second.clone();
        move || {
            let target = second.clone();
            [
                view! {
                    <unstyled::Button
                        tab_stop={create_memo(move || !skipped.get())}
                        on_key={move |press: KeyPress| {
                            if press.key != Key::ArrowRight || !press.pressed {
                                return false;
                            }
                            set_skipped.set(true);
                            crate::focus_within(target.get());
                            true
                        }}
                    >
                        <ButtonFace label="First" />
                    </unstyled::Button>
                },
                view! { <LabelledButton @node_ref=&second label="Second" /> },
            ]
        }
    });
    let flag = unstyled::button_focused(&document, second.get());
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert!(flag.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    assert!(flag.get());
}
