use super::*;
use crate::reactive::view;

#[test]
fn shift_tab_moves_focus_to_the_previous_button() {
    let (document, [first, second]) = toolbar_of(|| {
        [
            view! { <labelled_button label={"First".to_string()} /> },
            view! { <labelled_button label={"Second".to_string()} /> },
        ]
    });
    let first_focused = unstyled::button_focused(&document, first);
    let second_focused = unstyled::button_focused(&document, second);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::SHIFT);

    assert_eq!((first_focused.get(), second_focused.get()), (true, false));
}
