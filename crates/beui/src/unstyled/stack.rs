use beui_macros::{component, view};

use crate::base::{Direction, ItemSize};
use crate::node::NodeId;
use crate::reactive::{component_detail, Children, ListBuilder, Prop};

#[component]
pub fn stack(spacing: Prop<f32>, narrow: Prop<bool>, children: Children) -> NodeId {
    let stacked = narrow.memo();

    component_detail(stacked.map(|stacked| if stacked { "column" } else { "row" }.to_owned()));

    let direction = stacked.map(|stacked| {
        if stacked {
            Direction::Vertical
        } else {
            Direction::Horizontal
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
