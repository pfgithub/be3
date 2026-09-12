use beui_macros::{component, view};

use crate::node::NodeId;
use crate::reactive::{Child, FillBuilder, PaddingBuilder};
use crate::styled::theme::{CARD_RADIUS, SURFACE};
use crate::styled::BorderedBuilder;

const PADDING_HORIZONTAL: f32 = 18.0;
const PADDING_VERTICAL: f32 = 16.0;

#[component]
pub fn card(children: Child) -> NodeId {
    view! {
        <bordered corner_radius=CARD_RADIUS>
            <fill color=SURFACE radius=CARD_RADIUS>
                <padding horizontal=PADDING_HORIZONTAL vertical=PADDING_VERTICAL>
                    {children}
                </padding>
            </fill>
        </bordered>
    }
}
