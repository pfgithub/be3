use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Child, Fill, Outline};
use crate::styled::theme::{BORDER, BORDER_WIDTH};

#[component]
pub fn Bordered(corner_radius: u8, children: Child) -> NodeId {
    view! {
        <Outline color=BORDER width=BORDER_WIDTH radius={corner_radius} offset=0.0 visible=true>
            {children}
        </Outline>
    }
}

#[component]
pub fn Separator() -> NodeId {
    view! { <Fill color=BORDER radius=0></Fill> }
}
