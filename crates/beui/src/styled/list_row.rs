use beui_macros::{component, view};

use crate::color::Color32;

use crate::node::NodeId;
use crate::reactive::{Children, ClickCallback, FillBuilder, OutlineBuilder, PaddingBuilder, Prop};
use crate::styled::theme::{ACCENT, BORDER, RADIUS, SURFACE_RAISED};
use crate::unstyled;
use crate::unstyled::ButtonBuilder;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 4.0;

#[component]
pub fn list_row(children: Children, on_click: ClickCallback) -> NodeId {
    let child = children
        .into_first()
        .expect("list_row requires a child, e.g. <list_row>{content}</list_row>");

    view! {
        <button on_click={move || on_click.call()} content={Box::new(move |handle: unstyled::ButtonHandle| {
            let fill_color = Prop::Dynamic(Box::new(move || {
                background(handle.hovered.get(), handle.active.get())
            }));
            view! {
                <outline color={ACCENT} width={2.0} radius={RADIUS} offset={0.0} visible={handle.focused}>
                    <fill color={fill_color} radius={RADIUS}>
                        <padding horizontal={PADDING_HORIZONTAL} vertical={PADDING_VERTICAL}>{child}</padding>
                    </fill>
                </outline>
            }
        })} />
    }
}

fn background(hovered: bool, active: bool) -> Color32 {
    match (hovered, active) {
        (_, true) => BORDER,
        (true, false) => SURFACE_RAISED,
        (false, false) => Color32::TRANSPARENT,
    }
}
