use beui_macros::{component, view};

use crate::color::Color32;

use crate::node::NodeId;
use crate::reactive::{create_memo, Child, ClickCallback, Fill, Outline, Padding};
use crate::styled::theme::{ACCENT, BORDER, RADIUS, SURFACE_RAISED};
use crate::unstyled::{Button, ButtonHandle};

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 4.0;

#[component]
pub fn list_row(children: Child, on_click: ClickCallback) -> NodeId {
    view! {
        <Button
            on_click={move || on_click.call()}
            content={move |handle| view! { <ListRowFace handle>{children}</ListRowFace> }}
        />
    }
}

#[component]
fn list_row_face(handle: ButtonHandle, children: Child) -> NodeId {
    let ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let fill_color = create_memo(move || background(hovered.get(), active.get()));
    view! {
        <Outline color=ACCENT width=2.0 radius=RADIUS offset=0.0 visible={focused}>
            <Fill color={fill_color} radius=RADIUS>
                <Padding horizontal=PADDING_HORIZONTAL vertical=PADDING_VERTICAL>{children}</Padding>
            </Fill>
        </Outline>
    }
}

fn background(hovered: bool, active: bool) -> Color32 {
    match (hovered, active) {
        (_, true) => BORDER,
        (true, false) => SURFACE_RAISED,
        (false, false) => Color32::TRANSPARENT,
    }
}
