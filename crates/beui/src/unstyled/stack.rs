use beui_macros::{component, view};

use crate::base::{Direction, ItemSize};
use crate::node::NodeId;
use crate::reactive::{clone, component_detail, create_memo, Children, List, Prop};

#[component]
pub fn Stack(spacing: Prop<f32>, narrow: Prop<bool>, children: Children) -> NodeId {
    let stacked = create_memo(move || narrow.get());

    component_detail(create_memo(clone!(stacked -> move || {
        if stacked.get() { "column" } else { "row" }.to_owned()
    })));

    let direction = create_memo(clone!(stacked -> move || {
        if stacked.get() {
            Direction::Vertical
        } else {
            Direction::Horizontal
        }
    }));

    let children: Vec<(NodeId, Prop<ItemSize>)> = children
        .into_items()
        .into_iter()
        .map(|(child, size)| {
            let size = Prop::Dynamic(Box::new(clone!(stacked -> move || {
                if stacked.get() {
                    ItemSize::Intrinsic
                } else {
                    size.get()
                }
            })));
            (child, size)
        })
        .collect();

    view! { <List direction spacing children /> }
}
