use super::*;
use crate::reactive::{build, component, view, ColumnBuilder, RowBuilder};

#[component]
fn widget() -> NodeId {
    view! { <row spacing={0.0}></row> }
}

#[test]
fn a_component_function_appears_in_the_inspector_tree_without_hiding_its_children() {
    let document = build(|| {
        view! {
            <column spacing={0.0}>
                <widget />
            </column>
        }
    });
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(harness.tree(), ["column", "  widget", "    row"]);
}
