use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::Children;
use crate::styled::theme::NARROW_WIDTH;
use crate::unstyled;
use crate::unstyled::narrower_than;

#[component]
pub fn stack(
    spacing: f32,
    #[prop(default = NARROW_WIDTH)] breakpoint: f32,
    children: Children,
) -> NodeId {
    let narrow = narrower_than(breakpoint);
    view! { <unstyled::stack spacing={spacing} narrow={narrow} children={children} /> }
}
