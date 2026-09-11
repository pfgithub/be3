use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::TextInputBuilder;

#[test]
fn clicking_the_middle_of_a_placeholder_puts_the_caret_at_the_start() {
    let mut document = Document::new();
    let input = with_reactive_scope(&mut document, || {
        view! { <text_input value={String::new()} placeholder={"Search".to_string()} /> }
    });
    toolbar(&mut document, &[input]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(input));
    harness.type_text("ab");

    assert_eq!(styled::text_input_value(harness.document(), input), "ab");
}
