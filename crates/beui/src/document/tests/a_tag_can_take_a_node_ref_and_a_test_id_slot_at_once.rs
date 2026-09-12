use super::*;
use crate::reactive::{build, view, Column, NodeRef, Spacer};

#[test]
fn a_tag_can_take_a_node_ref_and_a_test_id_slot_at_once() {
    let spacer = NodeRef::new();
    let document = build({
        let spacer = spacer.clone();
        move || {
            view! {
                <Column spacing=0.0>
                    <Spacer @node_ref=&spacer @test_id="column.spacer" />
                </Column>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(harness.find("column.spacer"), spacer.get());
}
