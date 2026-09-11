use beui_macros::{component, view};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    component_detail, create_signal, set_component_state, untrack, with_document, Callback,
    Children, ColumnBuilder, NodeRef, Prop, ReadSignal, VisibilityBuilder,
};
use crate::unstyled;
use crate::unstyled::button::ButtonHandle;

pub struct DisclosureHandle {
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub open: ReadSignal<bool>,
}

pub type DisclosureHeader = Box<dyn FnOnce(DisclosureHandle) -> NodeId>;

struct State {
    button: NodeRef,
    open: ReadSignal<bool>,
}

#[component]
pub fn disclosure(
    spacing: f32,
    header: DisclosureHeader,
    children: Children,
    open: Prop<bool>,
    on_toggle: Callback<bool>,
) -> NodeId {
    let content = children
        .into_first()
        .expect("disclosure requires content, e.g. <unstyled::disclosure>{intrinsic(node)}</unstyled::disclosure>");

    let (open_read, set_open) = create_signal(false);
    open.apply({
        let set_open = set_open.clone();
        move |value| set_open.set(value)
    });

    component_detail({
        let open = open_read.clone();
        move || if open.get() { "open" } else { "closed" }.to_owned()
    });

    let open_for_header = open_read.clone();
    let open_for_click = open_read.clone();

    let button = NodeRef::new();

    let root = view! {
        <column spacing={spacing}>
            <unstyled::button
                node_ref={&button}
                on_click={move || {
                    let next = !untrack(|| open_for_click.get());
                    set_open.set(next);
                    on_toggle.call(next);
                }}
                content={Box::new(move |handle: ButtonHandle| {
                    header(DisclosureHandle {
                        hovered: handle.hovered,
                        active: handle.active,
                        focused: handle.focused,
                        open: open_for_header,
                    })
                })}
            />
            <visibility visible={open_read.clone()}>{content}</visibility>
        </column>
    };

    set_component_state(State {
        button,
        open: open_read,
    });

    root
}

pub fn disclosure_open(document: &Document, disclosure: NodeId) -> bool {
    document.component_state::<State>(disclosure).open.get()
}

pub fn disclosure_open_signal(document: &Document, disclosure: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(disclosure).open.clone()
}

pub fn disclosure_hovered(document: &Document, disclosure: NodeId) -> ReadSignal<bool> {
    let button = document.component_state::<State>(disclosure).button.get();
    unstyled::button_hovered(document, button)
}

pub fn disclosure_focused(document: &Document, disclosure: NodeId) -> ReadSignal<bool> {
    let button = document.component_state::<State>(disclosure).button.get();
    unstyled::button_focused(document, button)
}

pub fn focus_disclosure(disclosure: NodeId) {
    with_document(|document| {
        let button = document.component_state::<State>(disclosure).button.get();
        unstyled::focus_button(button);
    });
}
