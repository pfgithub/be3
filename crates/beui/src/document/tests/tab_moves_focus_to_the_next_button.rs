use super::*;
use crate::reactive::view;

#[test]
fn tab_moves_focus_to_the_next_button() {
    let (document, [first, second]) = toolbar_of(|| {
        [
            view! { <LabelledButton label="First" /> },
            view! { <LabelledButton label="Second" /> },
        ]
    });
    let first_focused = unstyled::button_focused(&document, first);
    let second_focused = unstyled::button_focused(&document, second);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!((first_focused.get(), second_focused.get()), (true, false));

    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!((first_focused.get(), second_focused.get()), (false, true));
}
