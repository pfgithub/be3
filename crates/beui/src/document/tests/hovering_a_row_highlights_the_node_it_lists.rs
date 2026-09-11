use super::*;

#[test]
fn hovering_a_row_highlights_the_node_it_lists() {
    let HelloColumn { document, text, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.frame(vec![Event::PointerMoved(harness.row_center(2))]);

    assert_eq!(harness.inspector().state.hovered.get(), Some(text));

    harness.frame(vec![Event::PointerMoved(pos2(4.0, 4.0))]);

    assert_eq!(harness.inspector().state.hovered.get(), None);
}
