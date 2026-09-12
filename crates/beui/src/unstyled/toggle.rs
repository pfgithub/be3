use beui_macros::{component, view};

use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    self, component_detail, create_signal, set_component_state, untrack, Callback,
    ClickCatcherBuilder, FocusableBuilder, Prop, ReadSignal, Render,
};

pub struct ToggleHandle {
    pub checked: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

#[component]
pub fn toggle(
    checked: Prop<bool>,
    content: Option<Render<ToggleHandle>>,
    on_change: Callback<bool>,
) -> NodeId {
    let (checked_read, set_checked) = checked.signal();
    let (hovered, set_hovered) = create_signal(false);
    let (active, set_active) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    let (key_active, set_key_active) = create_signal(false);

    component_detail({
        let checked = checked_read.clone();
        move || detail(checked.get()).to_owned()
    });

    let content_node = content.map(|build| {
        build.call(ToggleHandle {
            checked: checked_read.clone(),
            hovered: hovered.clone(),
            active: active.clone(),
            focused: focused.clone(),
        })
    });

    let toggle_checked = {
        let checked = checked_read.clone();
        move || {
            let next = !untrack(|| checked.get());
            set_checked.set(next);
            on_change.call(next);
        }
    };
    let key_toggle = toggle_checked.clone();

    set_component_state(checked_read.clone());

    view! {
        <focusable
            on_focus_change={move |focused: bool| set_focused.set(focused)}
            on_activate_change={move |pressed: bool| set_key_active.set(pressed)}
            on_activate={key_toggle}
        >
            <click_catcher
                cursor={CursorIcon::PointingHand}
                key_active={key_active}
                on_click={toggle_checked}
                on_hover_change={move |hovered: bool| set_hovered.set(hovered)}
                on_active_change={move |active: bool| set_active.set(active)}
                children={content_node.map(reactive::intrinsic)}
            />
        </focusable>
    }
}

pub fn toggle_checked(document: &Document, toggle: NodeId) -> ReadSignal<bool> {
    document.component_state::<ReadSignal<bool>>(toggle).clone()
}

fn detail(checked: bool) -> &'static str {
    if checked {
        "checked"
    } else {
        "unchecked"
    }
}
