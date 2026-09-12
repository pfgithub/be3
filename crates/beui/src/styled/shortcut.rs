use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{CenteredRowBuilder, ItemSize, Prop, TextBuilder};
use crate::styled::theme::{FONT_SMALL, TEXT_MUTED};
use crate::styled::ChipBuilder;

const SPACING: f32 = 10.0;

#[component]
pub fn shortcut(keys: Prop<String>, description: Prop<String>) -> NodeId {
    view! {
        <centered_row spacing=SPACING>
            <chip label={keys} />
            <text @sizing=ItemSize::Percent(100.0) string={description} font_size=FONT_SMALL color=TEXT_MUTED align=TextAlign::Start wrap=true />
        </centered_row>
    }
}
