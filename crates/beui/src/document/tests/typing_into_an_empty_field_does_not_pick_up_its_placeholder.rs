use super::*;
use crate::reactive::view;
use crate::styled::TextInput;

#[test]
fn typing_into_an_empty_field_does_not_pick_up_its_placeholder() {
    let (document, [input]) =
        toolbar_of(|| [view! { <TextInput value=String::new() placeholder="Search" /> }]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("a");

    assert_eq!(styled::text_input_value(harness.document(), input), "a");
}
