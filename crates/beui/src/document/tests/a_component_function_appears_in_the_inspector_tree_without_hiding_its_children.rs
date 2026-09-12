use super::*;
use crate::reactive::{build, component, view, Column, Row};

#[component]
fn Widget() -> NodeId {
    view! { <Row spacing=0.0></Row> }
}

#[test]
fn a_component_function_appears_in_the_inspector_tree_without_hiding_its_children() {
    let document = build(|| {
        view! {
            <Column spacing=0.0>
                <Widget />
            </Column>
        }
    });
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(harness.tree(), ["column", "  Widget", "    row"]);
}
