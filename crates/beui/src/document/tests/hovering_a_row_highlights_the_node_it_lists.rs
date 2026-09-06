use super::*;

#[test]
fn hovering_a_row_highlights_the_node_it_lists() {
    let mut document = Document::new();
    let text = document.create_text("Hello", 14.0, Color32::WHITE);
    let padding = document.create_padding(4.0, 4.0);
    document.set_padding_child(padding, text);
    let column = document.create_list(Direction::Vertical, 0.0);
    document.append_child(column, padding, ItemSize::Intrinsic);
    document.set_root(column);
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.frame(vec![Event::PointerMoved(harness.row_center(2))]);

    assert_eq!(harness.inspector().state.hovered.get(), Some(text));

    harness.frame(vec![Event::PointerMoved(pos2(4.0, 4.0))]);

    assert_eq!(harness.inspector().state.hovered.get(), None);
}
