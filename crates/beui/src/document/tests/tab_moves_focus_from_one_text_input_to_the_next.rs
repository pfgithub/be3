use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::TextInputBuilder;

#[test]
fn tab_moves_focus_from_one_text_input_to_the_next() {
    let mut document = Document::new();
    let (first, second) = with_reactive_scope(&mut document, || {
        (
            view! { <text_input value={String::new()} /> },
            view! { <text_input value={String::new()} /> },
        )
    });
    toolbar(&mut document, &[first, second]);
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
