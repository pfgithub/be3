use super::*;

#[test]
fn clicking_a_row_collapses_its_children() {
    let mut document = Document::new();
    let text = document.create_text("Hello", 14.0, Color32::WHITE);
    let padding = document.create_padding(4.0, 4.0);
    document.set_padding_child(padding, text);
    let column = document.create_list(Direction::Vertical, 0.0);
    document.append_child(column, padding, ItemSize::Intrinsic);
    document.set_root(column);
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    let padding_row = harness.row_center(1);
    harness.click(padding_row);
    harness.frame(Vec::new());

    assert_eq!(harness.tree(), ["column", "  padding"]);

    harness.click(padding_row);
    harness.frame(Vec::new());

    assert_eq!(harness.tree(), ["column", "  padding", "    text"]);
}
