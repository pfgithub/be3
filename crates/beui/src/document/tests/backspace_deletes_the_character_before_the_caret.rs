use super::*;
use crate::reactive::view;
use crate::styled::TextInputBuilder;

#[test]
fn backspace_deletes_the_character_before_the_caret() {
    let (document, [input]) = toolbar_of(|| [view! { <text_input value={"beui".to_string()} /> }]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Backspace, Modifiers::NONE);

    assert_eq!(styled::text_input_value(harness.document(), input), "beu");
}
