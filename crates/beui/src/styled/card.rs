use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Child, Fill, Padding};
use crate::styled::theme::{CARD_RADIUS, SURFACE};
use crate::styled::Bordered;

const PADDING_HORIZONTAL: f32 = 18.0;
const PADDING_VERTICAL: f32 = 16.0;

#[component]
pub fn card(children: Child) -> NodeId {
    view! {
        <Bordered corner_radius=CARD_RADIUS>
            <Fill color=SURFACE radius=CARD_RADIUS>
                <Padding horizontal=PADDING_HORIZONTAL vertical=PADDING_VERTICAL>
                    {children}
                </Padding>
            </Fill>
        </Bordered>
    }
}
