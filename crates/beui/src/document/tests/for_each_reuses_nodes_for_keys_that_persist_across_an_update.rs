use super::*;
use crate::reactive::{
    build, create_signal, intrinsic, view, ButtonBuilder, ColumnBuilder, ForEachBuilder,
    TextBuilder,
};

#[test]
fn for_each_reuses_nodes_for_keys_that_persist_across_an_update() {
    let list_id = std::rc::Rc::new(std::cell::Cell::new(None));
    let sink_list = list_id.clone();
    let shuffle_id = std::rc::Rc::new(std::cell::Cell::new(None));
    let sink_shuffle = shuffle_id.clone();

    let document = build(move || {
        let (items, set_items) = create_signal(vec![1i64, 2, 3]);

        let list = view! {
            <for_each
                spacing={0.0}
                items={items}
                key={Box::new(|value| *value)}
                view={Box::new(|value| {
                    intrinsic(view! { <text string={value.to_string()} /> })
                })}
            />
        };
        sink_list.set(Some(list));

        let shuffle = view! {
            <button on_click={Box::new(move |_document| set_items.set(vec![3, 2, 4]))}>
                <text string={"shuffle".to_string()} />
            </button>
        };
        sink_shuffle.set(Some(shuffle));

        view! {
            <column spacing={0.0}>
                {shuffle}
                {list}
            </column>
        }
    });

    let list = list_id.get().expect("the list container was created");
    let shuffle = shuffle_id.get().expect("the shuffle button was created");
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
