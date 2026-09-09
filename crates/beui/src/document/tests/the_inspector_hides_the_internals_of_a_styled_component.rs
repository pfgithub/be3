use super::*;
use crate::reactive::{view, with_document, with_reactive_scope};
use crate::styled::ListRowBuilder;

#[test]
fn the_inspector_hides_the_internals_of_a_styled_component() {
    let mut document = Document::new();
    let row = with_reactive_scope(&mut document, || {
        let label = with_document(|document| document.create_text("Hello", 14.0, Color32::WHITE));
        view! { <list_row>{label}</list_row> }
    });
    toolbar(&mut document, &[row]);
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(
        harness.tree(),
        [
            "column",
            "  list_row",
            "    button",
            "      shadow",
            "      content"
        ]
    );
}
