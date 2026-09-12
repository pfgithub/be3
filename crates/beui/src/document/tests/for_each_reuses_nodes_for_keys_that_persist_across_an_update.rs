use super::*;
use crate::reactive::{
    build, create_signal, view, ButtonBuilder, ColumnBuilder, ForEachBuilder, NodeRef, TextBuilder,
};

#[test]
fn for_each_reuses_nodes_for_keys_that_persist_across_an_update() {
    let (list, shuffle) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (list, shuffle) = (list.clone(), shuffle.clone());
        move || {
            let (items, set_items) = create_signal(vec![1i64, 2, 3]);
            view! {
                <column spacing=0.0>
                    <button
                        @node_ref=&shuffle
                        on_click={move || set_items.set(vec![3, 2, 4])}
                    >
                        <text string="shuffle" />
                    </button>
                    <for_each @node_ref=&list spacing=0.0 items key={|value: i64| value}>
                        {|value: i64| view! { <text string={value.to_string()} /> }}
                    </for_each>
                </column>
            }
        }
    });

    let (list, shuffle) = (list.get(), shuffle.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let list = harness.document().shadow_root(list);

    let before = harness.document().children(list);
    assert_eq!(
        before
            .iter()
            .map(|&id| text_of(harness.document(), id).to_owned())
            .collect::<Vec<_>>(),
        vec!["1", "2", "3"]
    );
    let node_for_two = before[1];
    let node_for_three = before[2];

    harness.click(harness.center(shuffle));
    harness.frame(Vec::new());

    let after = harness.document().children(list);
    assert_eq!(
        after
            .iter()
            .map(|&id| text_of(harness.document(), id).to_owned())
            .collect::<Vec<_>>(),
        vec!["3", "2", "4"],
        "surviving and new items must appear in the new order"
    );
    assert_eq!(
        after[0], node_for_three,
        "a key that persists must keep its node identity"
    );
    assert_eq!(
        after[1], node_for_two,
        "a key that persists must keep its node identity"
    );
    assert!(
        !before.contains(&after[2]),
        "a new key must get a freshly built node"
    );
}
