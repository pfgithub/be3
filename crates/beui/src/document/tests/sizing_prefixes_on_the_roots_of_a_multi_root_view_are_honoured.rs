use super::*;
use crate::reactive::{build, view, ColumnBuilder, NodeRef, RowBuilder};

#[test]
fn sizing_prefixes_on_the_roots_of_a_multi_root_view_are_honoured() {
    let (left, right) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (left, right) = (left.clone(), right.clone());
        move || {
            let panes = view! {
                @fixed(30.0) <column node_ref=&left spacing=0.0></column>
                @percent(100.0) <column node_ref=&right spacing=0.0></column>
            };
            view! { <row spacing=0.0 children={panes} /> }
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(left.get()).width(), 30.0);
    assert_eq!(
        harness.rect(right.get()).width(),
        WIDE_VIEWPORT.x - 30.0,
        "a sizing prefix on a fragment root must reach the row that takes the fragment"
    );
}
