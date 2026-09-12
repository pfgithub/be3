use super::*;
use crate::reactive::{build, view, ColumnBuilder, NodeRef, TextBuilder};

#[test]
fn a_multi_root_view_fills_a_children_prop_in_order() {
    let column = NodeRef::new();
    let (first, second, third) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build({
        let column = column.clone();
        let (first, second, third) = (first.clone(), second.clone(), third.clone());
        move || {
            let toolbar = view! {
                <text node_ref=&first string="One" font_size=14.0 color=Color32::WHITE />
                <text node_ref=&second string="Two" font_size=14.0 color=Color32::WHITE />
                {view! { <text node_ref=&third string="Three" font_size=14.0 color=Color32::WHITE /> }}
            };
            view! { <column node_ref=&column spacing=0.0 children={toolbar} /> }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().children(column.get()),
        vec![first.get(), second.get(), third.get()],
        "a multi-root view! must hand its roots to the children prop in source order"
    );
    assert_eq!(text_of(harness.document(), first.get()), "One");
    assert_eq!(text_of(harness.document(), third.get()), "Three");
}
