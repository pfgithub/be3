use super::*;
use crate::reactive::{build, column, component, intrinsic, row};

#[component]
fn widget() -> NodeId {
    row().spacing(0.0).children([]).build()
}

#[test]
fn a_component_function_appears_in_the_inspector_tree_without_hiding_its_children() {
    let document = build(|| {
        column()
            .spacing(0.0)
            .children([intrinsic(widget().build())])
            .build()
    });
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(harness.tree(), ["column", "  widget", "    row"]);
}
