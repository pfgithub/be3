use beui_macros::{component, view};

use crate::input::{CursorIcon, KeyPress};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    self, create_signal, set_component_state, untrack, Callback, Children, ClickCallback,
    ClickCatcherBuilder, FocusableBuilder, Prop, ReadSignal, Render,
};

pub struct ButtonHandle {
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

struct State {
    active: ReadSignal<bool>,
    focused: ReadSignal<bool>,
}

#[component]
pub fn button(
    children: Children,
    content: Option<Render<ButtonHandle>>,
    disabled: Prop<bool>,
    #[prop(default = true)] tab_stop: Prop<bool>,
    focused: Prop<bool>,
    on_click: ClickCallback,
    on_key: Callback<KeyPress, bool>,
    on_text: Callback<String>,
    on_focus_change: Callback<bool>,
) -> NodeId {
    let focus_request = focused;
    let (hovered, set_hovered) = create_signal(false);
    let (active, set_active) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    let (key_active, set_key_active) = create_signal(false);
    let disabled = disabled.memo();

    let content_node = match content {
        Some(build) => Some(build.call(ButtonHandle {
            hovered,
            active: active.clone(),
            focused: focused.clone(),
        })),
        None => children.into_first(),
    };

    let click = {
        let disabled = disabled.clone();
        move || {
            if untrack(|| disabled.get()) {
                return;
            }
            on_click.call();
        }
    };
    let key_click = click.clone();
    let tab_stop = tab_stop.map(move |tab_stop| tab_stop && !disabled.get());

    set_component_state(State {
        active: active.clone(),
        focused: focused.clone(),
    });

    view! {
        <focusable
            tab_stop={tab_stop}
            focused={focus_request}
            on_key={move |press| on_key.call(press)}
            on_text={move |text| on_text.call(text)}
            on_focus_change={move |has_focus: bool| {
                set_focused.set(has_focus);
                on_focus_change.call(has_focus);
            }}
            on_activate_change={move |pressed: bool| set_key_active.set(pressed)}
            on_activate={key_click}
        >
            <click_catcher
                cursor={CursorIcon::PointingHand}
                key_active={key_active}
                on_click={click}
                on_hover_change={move |hovered: bool| set_hovered.set(hovered)}
                on_active_change={move |active: bool| set_active.set(active)}
                children={content_node.map(reactive::intrinsic)}
            />
        </focusable>
    }
}

pub fn button_active(document: &Document, button: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(button).active.clone()
}

pub fn button_focused(document: &Document, button: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(button).focused.clone()
}
