use beui_macros::{component, view};

use crate::base::{Direction, ItemSize};
use crate::node::NodeId;
use crate::reactive::{component_detail, create_memo, Children, ListBuilder, Prop};

#[component]
pub fn stack(spacing: Prop<f32>, narrow: Prop<bool>, children: Children) -> NodeId {
    let stacked = narrow.memo();

    component_detail({
        let stacked = stacked.clone();
        move || if stacked.get() { "column" } else { "row" }.to_owned()
    });

    let direction = create_memo({
        let stacked = stacked.clone();
        move || {
            if stacked.get() {
                Direction::Vertical
            } else {
                Direction::Horizontal
            }
        }
    });

    let children: Vec<(NodeId, Prop<ItemSize>)> = children
        .into_items()
        .into_iter()
        .map(|(child, size)| {
            let stacked = stacked.clone();
            let size = size.reader();
            let size = Prop::Dynamic(Box::new(move || {
                if stacked.get() {
                    ItemSize::Intrinsic
                } else {
                    size()
                }
            }));
            (child, size)
        })
        .collect();

    view! { <list direction={direction} spacing={spacing} children={children} /> }
}
