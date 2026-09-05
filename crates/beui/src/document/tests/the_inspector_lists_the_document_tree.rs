use super::*;

#[test]
fn the_inspector_lists_the_document_tree() {
    let mut document = Document::new();
    let text = document.create_text("Hello", 14.0, Color32::WHITE);
    let padding = document.create_padding(4.0, 4.0);
    document.set_padding_child(padding, text);
    let column = document.create_list(Direction::Vertical, 0.0);
    document.append_child(column, padding, ItemSize::Intrinsic);
    document.set_root(column);
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(harness.tree(), ["column", "  padding", "    text"]);
}
