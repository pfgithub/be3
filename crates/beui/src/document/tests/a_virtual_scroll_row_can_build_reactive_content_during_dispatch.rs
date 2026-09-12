use super::*;
use crate::reactive::{build, view, ItemSize, NodeRef, Text, VirtualList};

#[test]
fn a_virtual_scroll_row_can_build_reactive_content_during_dispatch() {
    let scroll = NodeRef::new();
    let document = build({
        let scroll = scroll.clone();
        move || {
            view! {
                <Column spacing=0.0>
                    <VirtualList @sizing=ItemSize::Percent(100.0)
                        @node_ref=&scroll
                        count=VIRTUAL_ITEM_COUNT
                        item_height=VIRTUAL_ITEM_HEIGHT
                    >
                        {|index: usize| view! { <Text string={format!("Row {index}")} /> }}
                    </VirtualList>
                </Column>
            }
        }
    });
    let scroll = scroll.get();

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let first_row = harness.document.children(scroll)[0];
    assert_eq!(text_of(harness.document(), first_row), "Row 0");
}
