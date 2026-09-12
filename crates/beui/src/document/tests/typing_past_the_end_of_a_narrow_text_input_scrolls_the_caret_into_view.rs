use super::*;
use crate::reactive::{view, NodeRef, SizedBuilder};
use crate::styled::TextInputBuilder;

#[test]
fn typing_past_the_end_of_a_narrow_text_input_scrolls_the_caret_into_view() {
    let value = "a value that is much wider than the field";
    let input = NodeRef::new();
    let (document, [_sized]) = toolbar_of({
        let input = input.clone();
        move || {
            [view! {
                <sized width=80.0>
                    <text_input @node_ref=&input value=String::new() />
                </sized>
            }]
        }
    });
    let input = input.get();
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text(value);
    harness.frame(Vec::new());

    let inner = harness.document().shadow_root(input);
    let text = unstyled::text_input_text(harness.document(), inner);
    let rect = harness.rect(text);
    let caret = harness
        .document()
        .text_index_at(text, pos2(rect.right(), rect.center().y));

    assert_eq!(caret, value.len());
}
