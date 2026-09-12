use beui_macros::{component, view};

use crate::input::CursorIcon;

use crate::node::NodeId;
use crate::reactive::{
    self, clone, create_memo, create_signal, untrack, Callback, Child, ClickCallback, ClickCatcher,
    Focusable, Prop,
};

#[component]
pub fn pressable(
    children: Option<Child>,
    #[prop(default = true)] enabled: Prop<bool>,
    on_click: ClickCallback,
    on_focus_change: Callback<bool>,
    on_hover_change: Callback<bool>,
    on_active_change: Callback<bool>,
) -> NodeId {
    let enabled = create_memo(move || enabled.get());
    let (key_active, set_key_active) = create_signal(false);
    let click = clone!(enabled -> move || {
        if untrack(|| enabled.get()) {
            on_click.call();
        }
    });
    let key_click = click.clone();

    view! {
        <Focusable
            tab_stop={enabled}
            on_focus_change={move |focused| on_focus_change.call(focused)}
            on_activate_change={move |pressed| set_key_active.set(pressed)}
            on_activate={key_click}
        >
            <ClickCatcher
                cursor=CursorIcon::PointingHand
                key_active
                on_click={click}
                on_hover_change={move |hovered| on_hover_change.call(hovered)}
                on_active_change={move |active| on_active_change.call(active)}
                children={children.map(reactive::intrinsic)}
            />
        </Focusable>
    }
}
