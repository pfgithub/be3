use super::*;
use crate::reactive::{build, column, component, intrinsic, row};

#[component]
fn widget() -> NodeId {
    row(0.0, [])
}

#[test]
fn a_component_function_appears_in_the_inspector_tree_without_hiding_its_children() {
    let document = build(|| column(0.0, [intrinsic(widget())]));
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(harness.tree(), ["column", "  widget", "    row"]);
}
