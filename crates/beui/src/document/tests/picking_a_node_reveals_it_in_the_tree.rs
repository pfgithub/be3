use super::*;

#[test]
fn picking_a_node_reveals_it_in_the_tree() {
    let mut document = Document::new();
    let text = document.create_text("Hello", 14.0, Color32::WHITE);
    let mut child = text;
    for _ in 0..4 {
        let padding = document.create_padding(4.0, 4.0);
        document.set_padding_child(padding, child);
        child = padding;
    }
    let column = document.create_list(Direction::Vertical, 0.0);
    document.append_child(column, child, ItemSize::Intrinsic);
    document.set_root(column);
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    assert_eq!(
        harness.tree(),
        ["column", "  padding", "    padding", "      padding"]
    );

    harness.toggle_picking();
    assert!(harness.inspector().state.picking.get());

    let target = harness
        .document
        .node_rect(text)
        .expect("the text was not laid out")
        .center();
    harness.click(target);
    harness.frame(Vec::new());

    assert!(!harness.inspector().state.picking.get());
    assert_eq!(harness.inspector().state.selected.get(), Some(text));
    assert_eq!(
        harness.tree(),
        [
            "column",
            "  padding",
            "    padding",
            "      padding",
            "        padding",
            "          text",
        ]
    );
}
