use super::*;
use crate::reactive::view;
use crate::styled::TextInput;

#[test]
fn typing_into_a_focused_text_input_inserts_the_text() {
    let (document, [input]) = toolbar_of(|| [view! { <TextInput value=String::new() /> }]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("beui");

    assert_eq!(styled::text_input_value(harness.document(), input), "beui");
}
