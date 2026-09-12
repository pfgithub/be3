use super::*;
use crate::reactive::{build, ItemSize, NodeRef, Padding, Row};

#[test]
fn percent_sized_children_still_size_an_intrinsic_lists_height() {
    let row = NodeRef::new();
    let document = build({
        let row = row.clone();
        move || {
            view! {
                <Column spacing=0.0>
                    <Row @node_ref=&row spacing=0.0>
                        <Padding @sizing=ItemSize::Percent(50.0) horizontal=0.0 vertical=20.0><Spacer /></Padding>
                        <Padding @sizing=ItemSize::Percent(50.0) horizontal=0.0 vertical=20.0><Spacer /></Padding>
                    </Row>
                </Column>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(row.get()).height(), 40.0);
}
