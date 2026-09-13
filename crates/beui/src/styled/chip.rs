use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{create_memo, Fill, Padding, Prop, Text};
use crate::styled::theme::{CHIP_RADIUS, FONT_SMALL, SURFACE_RAISED, TEXT};
use crate::styled::Bordered;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 3.0;

#[component]
pub fn Chip(label: Prop<String>) -> NodeId {
    let label_text = create_memo(move || label.get());
    view! {
        <Bordered corner_radius=CHIP_RADIUS>
            <Fill color=SURFACE_RAISED radius=CHIP_RADIUS>
                <Padding horizontal=PADDING_HORIZONTAL vertical=PADDING_VERTICAL>
                    <Text string={label_text} font_size=FONT_SMALL color=TEXT align=TextAlign::Center />
                </Padding>
            </Fill>
        </Bordered>
    }
}
