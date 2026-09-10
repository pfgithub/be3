use super::*;
use crate::reactive::{view, TextBuilder};

#[test]
fn a_virtual_scroll_row_can_build_reactive_content_during_dispatch() {
    let mut document = Document::new();
    let scroll = document.create_scroll();
    document.set_scroll_virtual_items(
        scroll,
        VIRTUAL_ITEM_COUNT,
        VIRTUAL_ITEM_HEIGHT,
        move |_document, index| view! { <text string={format!("Row {index}")} /> },
    );
    document.set_root(scroll);

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let first_row = harness.document.children(scroll)[0];
    assert_eq!(text_of(harness.document(), first_row), "Row 0");
}
