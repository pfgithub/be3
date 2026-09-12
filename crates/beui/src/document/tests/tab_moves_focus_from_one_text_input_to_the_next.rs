use super::*;
use crate::reactive::view;
use crate::styled::TextInput;

#[test]
fn tab_moves_focus_from_one_text_input_to_the_next() {
    let (document, [first, second]) = toolbar_of(|| {
        [
            view! { <TextInput value=String::new() /> },
            view! { <TextInput value=String::new() /> },
        ]
    });
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("second");

    assert_eq!(styled::text_input_value(harness.document(), first), "");
    assert_eq!(
        styled::text_input_value(harness.document(), second),
        "second"
    );
}
