use super::*;

#[test]
fn the_inspector_follows_nodes_added_to_the_document() {
    let mut document = Document::new();
    let first = document.create_text("First", 14.0, Color32::WHITE);
    let column = document.create_list(Direction::Vertical, 0.0);
    document.append_child(column, first, ItemSize::Intrinsic);
    document.set_root(column);
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    assert_eq!(harness.tree(), ["column", "  text"]);

    let second = harness.document.create_text("Second", 14.0, Color32::WHITE);
    harness
        .document
        .append_child(column, second, ItemSize::Intrinsic);
    harness.frame(Vec::new());

    assert_eq!(harness.tree(), ["column", "  text", "  text"]);

    harness.document.remove_child(column, first);
    harness.frame(Vec::new());

    assert_eq!(harness.tree(), ["column", "  text"]);
}
