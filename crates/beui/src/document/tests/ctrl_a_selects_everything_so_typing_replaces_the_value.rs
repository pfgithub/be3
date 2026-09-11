use super::*;
use crate::reactive::view;
use crate::styled::TextInputBuilder;

#[test]
fn ctrl_a_selects_everything_so_typing_replaces_the_value() {
    let (document, [input]) = toolbar_of(|| [view! { <text_input value={"hello".to_string()} /> }]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::A, Modifiers::CTRL);
    harness.type_text("bye");

    assert_eq!(styled::text_input_value(harness.document(), input), "bye");
}
