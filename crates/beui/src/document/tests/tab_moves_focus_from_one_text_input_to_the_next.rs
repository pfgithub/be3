use super::*;
use crate::reactive::view;
use crate::styled::TextInputBuilder;

#[test]
fn tab_moves_focus_from_one_text_input_to_the_next() {
    let (document, [first, second]) = toolbar_of(|| {
        [
            view! { <text_input value=String::new() /> },
            view! { <text_input value=String::new() /> },
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
