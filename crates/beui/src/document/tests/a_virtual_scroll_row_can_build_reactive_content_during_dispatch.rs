use super::*;
use crate::reactive::{build, view, NodeRef, TextBuilder, VirtualListBuilder};

#[test]
fn a_virtual_scroll_row_can_build_reactive_content_during_dispatch() {
    let scroll = NodeRef::new();
    let document = build({
        let scroll = scroll.clone();
        move || {
            view! {
                <column spacing=0.0>
                    @percent(100.0) <virtual_list
                        node_ref=&scroll
                        count=VIRTUAL_ITEM_COUNT
                        item_height=VIRTUAL_ITEM_HEIGHT
                    >
                        {|index: usize| view! { <text string={format!("Row {index}")} /> }}
                    </virtual_list>
                </column>
            }
        }
    });
    let scroll = scroll.get();

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let first_row = harness.document.children(scroll)[0];
    assert_eq!(text_of(harness.document(), first_row), "Row 0");
}
