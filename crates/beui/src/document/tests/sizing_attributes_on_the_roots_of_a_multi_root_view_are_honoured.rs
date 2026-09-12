use super::*;
use crate::reactive::{build, view, Column, ItemSize, NodeRef, Row};

#[test]
fn sizing_attributes_on_the_roots_of_a_multi_root_view_are_honoured() {
    let (left, right) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (left, right) = (left.clone(), right.clone());
        move || {
            let panes = view! {
                <Column @sizing=ItemSize::Fixed(30.0) @node_ref=&left spacing=0.0></Column>
                <Column @sizing=ItemSize::Percent(100.0) @node_ref=&right spacing=0.0></Column>
            };
            view! { <Row spacing=0.0 children={panes} /> }
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(left.get()).width(), 30.0);
    assert_eq!(
        harness.rect(right.get()).width(),
        WIDE_VIEWPORT.x - 30.0,
        "a `@sizing` attribute on a fragment root must reach the row that takes the fragment"
    );
}
