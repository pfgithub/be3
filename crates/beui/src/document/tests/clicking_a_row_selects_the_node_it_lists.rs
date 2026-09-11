use super::*;

#[test]
fn clicking_a_row_selects_the_node_it_lists() {
    let HelloColumn {
        document, padding, ..
    } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    assert_eq!(harness.inspector().state.selected.get(), None);

    harness.click(harness.row_center(1));
    harness.frame(Vec::new());

    assert_eq!(harness.inspector().state.selected.get(), Some(padding));
    assert_eq!(harness.tree(), ["column", "  padding", "    text"]);
}
