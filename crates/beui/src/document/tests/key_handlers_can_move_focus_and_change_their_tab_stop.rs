use super::*;
use crate::reactive::{create_memo, create_signal};
use crate::KeyPress;

#[test]
fn key_handlers_can_move_focus_and_change_their_tab_stop() {
    let mut document = Document::new();
    let (skipped, set_skipped) = create_signal(false);
    let second = labelled_button(&mut document, "Second");
    let flag = unstyled::button_focused(&document, second);
    let first = with_installed(&mut document, |_| {
        view! {
            <unstyled::button
                tab_stop={create_memo(move || !skipped.get())}
                on_key={move |press: KeyPress| {
                    if press.key != Key::ArrowRight || !press.pressed {
                        return false;
                    }
                    set_skipped.set(true);
                    unstyled::focus_button(second);
                    true
                }}
            >
                <button_face label={"First".to_string()} />
            </unstyled::button>
        }
    });
    toolbar(&mut document, &[first, second]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert!(flag.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    assert!(flag.get());
}
