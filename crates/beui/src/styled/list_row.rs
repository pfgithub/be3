use beui_macros::{component, view};

use crate::color::Color32;

use crate::node::{ClickHandler, NodeId};
use crate::reactive::{bind, with_document, Children, FillBuilder, OutlineBuilder, PaddingBuilder};
use crate::styled::theme::{ACCENT, BORDER, RADIUS, SURFACE_RAISED};
use crate::unstyled;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 4.0;

#[component]
pub fn list_row(children: Children, on_click: Option<ClickHandler>) -> NodeId {
    let child = children
        .into_first()
        .expect("list_row requires a child, e.g. <list_row>{content}</list_row>");

    let row = unstyled::ButtonBuilder::default().build();
    let hovered = with_document(|document| unstyled::button_hovered(document, row));
    let active = with_document(|document| unstyled::button_active(document, row));
    let focused = with_document(|document| unstyled::button_focused(document, row));

    let fill = view! {
        <fill color={Color32::TRANSPARENT} radius={RADIUS}>
            <padding horizontal={PADDING_HORIZONTAL} vertical={PADDING_VERTICAL}>{child}</padding>
        </fill>
    };
    let ring = view! {
        <outline color={ACCENT} width={2.0} radius={RADIUS} offset={0.0} visible={focused}>
            {fill}
        </outline>
    };
    unstyled::set_button_child(row, ring);

    bind(move |document| {
        document.set_fill_color(fill, background(hovered.get(), active.get()));
    });

    if let Some(on_click) = on_click {
        unstyled::set_button_on_click(row, on_click);
    }

    row
}

fn background(hovered: bool, active: bool) -> Color32 {
    match (hovered, active) {
        (_, true) => BORDER,
        (true, false) => SURFACE_RAISED,
        (false, false) => Color32::TRANSPARENT,
    }
}
