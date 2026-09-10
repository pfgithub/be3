use beui_macros::{component, view};

use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    self, component_detail, create_signal, current_component, set_component_state, untrack,
    with_document, Callback, ClickCatcherBuilder, FocusableBuilder, Prop, ReadSignal,
};

pub struct ToggleHandle {
    pub checked: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub type ToggleContent = Box<dyn FnOnce(ToggleHandle) -> NodeId>;

struct State {
    focusable: NodeId,
    checked: ReadSignal<bool>,
    hovered: ReadSignal<bool>,
    active: ReadSignal<bool>,
    focused: ReadSignal<bool>,
}

#[component]
pub fn toggle(
    checked: Prop<bool>,
    content: Option<ToggleContent>,
    on_change: Callback<bool>,
) -> NodeId {
    let toggle = current_component();
    let (checked_read, set_checked) = create_signal(false);
    checked.apply({
        let set_checked = set_checked.clone();
        move |value| set_checked.set(value)
    });
    let (hovered, set_hovered) = create_signal(false);
    let (active, set_active) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    let (key_active, set_key_active) = create_signal(false);

    component_detail({
        let checked = checked_read.clone();
        move || detail(checked.get()).to_owned()
    });

    let content_node = content.map(|build| {
        build(ToggleHandle {
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

    let focusable = view! {
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
    };

    with_document(|document| {
        set_component_state(
            document,
            toggle,
            State {
                focusable,
                checked: checked_read,
                hovered,
                active,
                focused,
            },
        );
    });

    focusable
}

pub fn toggle_checked(document: &Document, toggle: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(toggle).checked.clone()
}

pub fn toggle_hovered(document: &Document, toggle: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(toggle).hovered.clone()
}

pub fn toggle_active(document: &Document, toggle: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(toggle).active.clone()
}

pub fn toggle_focused(document: &Document, toggle: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(toggle).focused.clone()
}

fn detail(checked: bool) -> &'static str {
    if checked {
        "checked"
    } else {
        "unchecked"
    }
}

pub fn focus_toggle(document: &mut Document, toggle: NodeId) {
    let focusable = document.component_state::<State>(toggle).focusable;
    document.focus_focusable(focusable);
}
