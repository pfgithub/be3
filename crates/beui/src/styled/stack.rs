use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Children, Prop};
use crate::styled::theme::NARROW_WIDTH;
use crate::unstyled;
use crate::unstyled::narrower_than;

#[component]
pub fn Stack(
    spacing: Prop<f32>,
    #[prop(default = NARROW_WIDTH)] breakpoint: f32,
    children: Children,
) -> NodeId {
    let narrow = narrower_than(breakpoint);
    view! { <unstyled::Stack spacing narrow children /> }
}
