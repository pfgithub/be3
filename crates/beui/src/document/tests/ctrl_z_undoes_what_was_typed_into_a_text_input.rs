use super::*;
use crate::reactive::view;
use crate::styled::TextInput;

#[test]
fn ctrl_z_undoes_what_was_typed_into_a_text_input() {
    let (document, [input]) = toolbar_of(|| [view! { <TextInput value="note" /> }]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("s");
    harness.key(Key::Z, Modifiers::CTRL);

    assert_eq!(styled::text_input_value(harness.document(), input), "note");
}
