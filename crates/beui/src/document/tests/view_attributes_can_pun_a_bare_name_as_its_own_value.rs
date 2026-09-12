use super::*;
use crate::reactive::view;

const PUNNED: &str = "Punned";

#[test]
fn view_attributes_can_pun_a_bare_name_as_its_own_value() {
    let column = NodeRef::new();
    let label = NodeRef::new();
    let document = build({
        let column = column.clone();
        let label = label.clone();
        move || {
            let string = PUNNED.to_string();
            let font_size = 18.0f32;
            let spacing = 0.0f32;
            let children = [intrinsic(view! {
                <text node_ref={&label} string font_size />
            })];
            view! { <column node_ref={&column} spacing children /> }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), label.get()), PUNNED);
    assert_eq!(harness.document().children(column.get()), vec![label.get()]);
}
