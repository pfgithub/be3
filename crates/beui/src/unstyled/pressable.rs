use beui_macros::{component, view};

use crate::input::CursorIcon;

use crate::node::NodeId;
use crate::reactive::{
    self, create_signal, Callback, Children, ClickCallback, ClickCatcherBuilder, FocusableBuilder,
};

#[component]
pub fn pressable(
    children: Children,
    on_click: ClickCallback,
    on_focus_change: Callback<bool>,
    on_hover_change: Callback<bool>,
    on_active_change: Callback<bool>,
) -> NodeId {
    let child = children.into_first();
    let (key_active, set_key_active) = create_signal(false);
    let key_click = on_click.clone();

    view! {
        <focusable
            on_focus_change={move |focused| on_focus_change.call(focused)}
            on_activate_change={move |pressed| set_key_active.set(pressed)}
            on_activate={move || key_click.call()}
        >
            <click_catcher
                cursor={CursorIcon::PointingHand}
                key_active={key_active}
                on_click={move || on_click.call()}
                on_hover_change={move |hovered| on_hover_change.call(hovered)}
                on_active_change={move |active| on_active_change.call(active)}
                children={child.map(reactive::intrinsic)}
            />
        </focusable>
    }
}
