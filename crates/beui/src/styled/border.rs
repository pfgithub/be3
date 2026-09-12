use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Child, FillBuilder, OutlineBuilder};
use crate::styled::theme::{BORDER, BORDER_WIDTH};

#[component]
pub fn bordered(corner_radius: u8, children: Child) -> NodeId {
    view! {
        <outline color={BORDER} width={BORDER_WIDTH} radius={corner_radius} offset={0.0} visible={true}>
            {children}
        </outline>
    }
}

#[component]
pub fn separator() -> NodeId {
    view! { <fill color={BORDER} radius={0}></fill> }
}
