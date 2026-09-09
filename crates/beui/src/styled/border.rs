use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Children, FillBuilder, OutlineBuilder};
use crate::styled::theme::{BORDER, BORDER_WIDTH};

#[component]
pub fn bordered(corner_radius: u8, children: Children) -> NodeId {
    let child = children
        .into_first()
        .expect("bordered requires a child, e.g. <bordered>{content}</bordered>");
    view! {
        <outline color={BORDER} width={BORDER_WIDTH} radius={corner_radius} offset={0.0} visible={true}>
            {child}
        </outline>
    }
}

#[component]
pub fn separator() -> NodeId {
    view! { <fill color={BORDER} radius={0}></fill> }
}
