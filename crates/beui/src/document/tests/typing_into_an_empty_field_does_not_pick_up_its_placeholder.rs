use super::*;
use crate::reactive::view;
use crate::styled::TextInputBuilder;

#[test]
fn typing_into_an_empty_field_does_not_pick_up_its_placeholder() {
    let (document, [input]) = toolbar_of(|| {
        [view! { <text_input value={String::new()} placeholder={"Search".to_string()} /> }]
    });
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("a");

    assert_eq!(styled::text_input_value(harness.document(), input), "a");
}
