use super::*;
use crate::reactive::{create_signal, view, with_reactive_scope};
use crate::styled::CaptionBuilder;

#[test]
fn removing_a_node_stops_the_effects_that_were_built_for_it() {
    let mut document = Document::new();
    let (label, set_label) = with_reactive_scope(&mut document, || create_signal("one".to_owned()));
    let caption = with_reactive_scope(&mut document, || {
        view! { <caption content={label} /> }
    });
    let list = toolbar(&mut document, &[caption]);

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.document_mut().remove_child(list, caption);
    harness.document_mut().remove_node(caption);
    with_installed(harness.document_mut(), |_| set_label.set("two".to_owned()));
    harness.frame(Vec::new());
}
