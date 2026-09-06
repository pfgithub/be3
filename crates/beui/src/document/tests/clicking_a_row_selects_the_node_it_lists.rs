use super::*;

#[test]
fn clicking_a_row_selects_the_node_it_lists() {
    let mut document = Document::new();
    let text = document.create_text("Hello", 14.0, Color32::WHITE);
    let padding = document.create_padding(4.0, 4.0);
    document.set_padding_child(padding, text);
    let column = document.create_list(Direction::Vertical, 0.0);
    document.append_child(column, padding, ItemSize::Intrinsic);
    document.set_root(column);
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    assert_eq!(harness.inspector().state.selected.get(), None);

    harness.click(harness.row_center(1));
    harness.frame(Vec::new());

    assert_eq!(harness.inspector().state.selected.get(), Some(padding));
    assert_eq!(harness.tree(), ["column", "  padding", "    text"]);
}
