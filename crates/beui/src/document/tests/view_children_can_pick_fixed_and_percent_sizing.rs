use super::*;
use crate::reactive::{build, view, with_document, ColumnBuilder, RowBuilder};

#[test]
fn view_children_can_pick_fixed_and_percent_sizing() {
    let left = Rc::new(Cell::new(None));
    let right = Rc::new(Cell::new(None));
    let sink_left = left.clone();
    let sink_right = right.clone();

    let document = build(move || {
        let tree = view! {
            <row spacing={0.0}>
                @fixed(30.0) <column spacing={0.0}></column>
                @percent(100.0) <column spacing={0.0}></column>
            </row>
        };
        let children = with_document(|document| document.children(tree));
        sink_left.set(Some(children[0]));
        sink_right.set(Some(children[1]));
        tree
    });

    let left = left.get().expect("fixed child was created");
    let right = right.get().expect("percent child was created");

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(left).width(), 30.0);
    assert_eq!(harness.rect(right).width(), WIDE_VIEWPORT.x - 30.0);
}
