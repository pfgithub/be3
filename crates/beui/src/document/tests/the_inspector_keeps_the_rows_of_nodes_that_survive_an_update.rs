use super::*;

#[test]
fn the_inspector_keeps_the_rows_of_nodes_that_survive_an_update() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.frame(Vec::new());
    let column_row = harness.inspector().row_node(0);
    let padding_row = harness.inspector().row_node(1);

    harness.click(harness.marker_center(1));
    harness.frame(Vec::new());

    assert_eq!(harness.tree(), ["column", "  padding"]);
    assert_eq!(
        (
            harness.inspector().row_node(0),
            harness.inspector().row_node(1)
        ),
        (column_row, padding_row),
        "collapsing a row must update the rows around it rather than rebuild them"
    );

    harness.click(harness.marker_center(1));
    harness.frame(Vec::new());

    assert_eq!(harness.tree(), ["column", "  padding", "    text"]);
    assert_eq!(
        (
            harness.inspector().row_node(0),
            harness.inspector().row_node(1)
        ),
        (column_row, padding_row)
    );
}
