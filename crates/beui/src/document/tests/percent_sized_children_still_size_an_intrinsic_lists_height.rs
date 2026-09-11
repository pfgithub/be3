use super::*;
use crate::reactive::{build, NodeRef, PaddingBuilder, RowBuilder};

#[test]
fn percent_sized_children_still_size_an_intrinsic_lists_height() {
    let row = NodeRef::new();
    let document = build({
        let row = row.clone();
        move || {
            view! {
                <column spacing={0.0}>
                    <row node_ref={&row} spacing={0.0}>
                        @percent(50.0) <padding horizontal={0.0} vertical={20.0}><spacer /></padding>
                        @percent(50.0) <padding horizontal={0.0} vertical={20.0}><spacer /></padding>
                    </row>
                </column>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(row.get()).height(), 40.0);
}
