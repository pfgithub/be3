use beui_macros::{component, view};

use crate::input::{CursorIcon, KeyPress};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    self, create_signal, set_component_state, untrack, with_document, Callback, Children,
    ClickCallback, ClickCatcherBuilder, FocusableBuilder, Prop, ReadSignal,
};

pub struct ButtonHandle {
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub type ButtonContent = Box<dyn FnOnce(ButtonHandle) -> NodeId>;

struct State {
    focusable: NodeId,
    hovered: ReadSignal<bool>,
    active: ReadSignal<bool>,
    focused: ReadSignal<bool>,
    on_click: ClickCallback,
}

#[component]
pub fn button(
    children: Children,
    content: Option<ButtonContent>,
    disabled: Prop<bool>,
    #[prop(default = true)] tab_stop: Prop<bool>,
    on_click: ClickCallback,
    on_key: Callback<KeyPress, bool>,
    on_text: Callback<String>,
) -> NodeId {
    let (hovered, set_hovered) = create_signal(false);
    let (active, set_active) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    let (key_active, set_key_active) = create_signal(false);
    let (disabled_read, set_disabled) = create_signal(false);
    disabled.apply(move |disabled| set_disabled.set(disabled));

    let content_node = match content {
        Some(build) => Some(build(ButtonHandle {
            hovered: hovered.clone(),
            active: active.clone(),
            focused: focused.clone(),
        })),
        None => children.into_first(),
    };

    let click = {
        let on_click = on_click.clone();
        let disabled = disabled_read.clone();
        move || {
            if untrack(|| disabled.get()) {
                return;
            }
            on_click.call();
        }
    };
    let key_click = click.clone();
    let tab_stop = {
        let disabled = disabled_read.clone();
        tab_stop.map(move |tab_stop| tab_stop && !disabled.get())
    };

    let focusable = view! {
        <focusable
            tab_stop={tab_stop}
            on_key={move |press| on_key.call(press)}
            on_text={move |text| on_text.call(text)}
            on_focus_change={move |focused: bool| set_focused.set(focused)}
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
    };

    set_component_state(State {
        focusable,
        hovered,
        active,
        focused,
        on_click,
    });

    focusable
}

pub fn button_hovered(document: &Document, button: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(button).hovered.clone()
}

pub fn button_active(document: &Document, button: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(button).active.clone()
}

pub fn button_focused(document: &Document, button: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(button).focused.clone()
}

pub fn set_button_on_click(button: NodeId, handler: impl FnMut() + 'static) {
    with_document(|document| {
        document
            .component_state::<State>(button)
            .on_click
            .set(handler);
    });
}

pub fn focus_button(button: NodeId) {
    with_document(|document| {
        let focusable = document.component_state::<State>(button).focusable;
        document.focus_focusable(focusable);
    });
}

pub fn button_focusable(document: &Document, button: NodeId) -> NodeId {
    document.component_state::<State>(button).focusable
}
