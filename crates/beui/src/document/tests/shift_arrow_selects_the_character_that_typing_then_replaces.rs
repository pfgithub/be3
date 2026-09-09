use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::TextInputBuilder;

#[test]
fn shift_arrow_selects_the_character_that_typing_then_replaces() {
    let mut document = Document::new();
    let input = with_reactive_scope(&mut document, || {
        view! { <text_input value={"cat".to_string()} /> }
    });
    toolbar(&mut document, &[input]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowLeft, Modifiers::SHIFT);
    harness.type_text("r");

    assert_eq!(styled::text_input_value(harness.document(), input), "car");
}
