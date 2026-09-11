use super::*;
use crate::reactive::view;
use crate::styled::TextInputBuilder;

#[test]
fn an_empty_field_shows_its_placeholder_until_something_is_typed() {
    let (document, [input]) = toolbar_of(|| {
        [view! { <text_input value={String::new()} placeholder={"Search".to_string()} /> }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let inner = harness.document().shadow_root(input);
    let text = unstyled::text_input_text(harness.document(), inner);

    assert_eq!(text_of(harness.document(), text), "Search");

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("a");
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), text), "a");
}
