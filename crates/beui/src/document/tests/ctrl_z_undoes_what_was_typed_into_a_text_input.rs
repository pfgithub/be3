use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::TextInputBuilder;

#[test]
fn ctrl_z_undoes_what_was_typed_into_a_text_input() {
    let mut document = Document::new();
    let input = with_reactive_scope(&mut document, || {
        view! { <text_input value={"note".to_string()} /> }
    });
    toolbar(&mut document, &[input]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("s");
    harness.key(Key::Z, Modifiers::CTRL);

    assert_eq!(styled::text_input_value(harness.document(), input), "note");
}
