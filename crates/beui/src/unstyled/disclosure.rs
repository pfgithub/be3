use beui_macros::{component, view};

use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    create_effect, create_signal, current_component, set_component_detail, set_component_state,
    with_document, Children, ColumnBuilder, Prop, ReadSignal, VisibilityBuilder,
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
    button: NodeId,
    open_read: ReadSignal<bool>,
    on_toggle: Option<Handler<bool>>,
}

#[component]
pub fn disclosure(
    spacing: f32,
    header: DisclosureHeader,
    children: Children,
    open: Prop<bool>,
) -> NodeId {
    let disclosure = current_component();
    let content = children
        .into_first()
        .expect("disclosure requires content, e.g. <unstyled::disclosure>{intrinsic(node)}</unstyled::disclosure>");

    let (open_read, open_write) = create_signal(false);
    open.apply({
        let open_write = open_write.clone();
        move |value| open_write.set(value)
    });

    create_effect({
        let open_read = open_read.clone();
        move || {
            let value = open_read.get();
            with_document(|document| {
                set_component_detail(document, disclosure, if value { "open" } else { "closed" });
            });
        }
    });

    let open_for_header = open_read.clone();
    let button = view! {
        <unstyled::button content={Box::new(move |handle: ButtonHandle| {
            header(DisclosureHandle {
                hovered: handle.hovered,
                active: handle.active,
                focused: handle.focused,
                open: open_for_header,
            })
        })} />
    };

    let root = view! {
        <column spacing={spacing}>
            {button}
            <visibility visible={open_read.clone()}>{content}</visibility>
        </column>
    };

    with_document(|document| {
        set_component_state(
            document,
            disclosure,
            State {
                button,
                open_read: open_read.clone(),
                on_toggle: None,
            },
        );
    });

    unstyled::set_button_on_click(button, move |document| {
        let next = !open_read.get();
        open_write.set(next);
        document.call_component_handler(disclosure, next, |state: &mut State| &mut state.on_toggle);
    });

    root
}

pub fn disclosure_open(document: &Document, disclosure: NodeId) -> bool {
    document
        .component_state::<State>(disclosure)
        .open_read
        .get()
}

pub fn disclosure_open_signal(document: &Document, disclosure: NodeId) -> ReadSignal<bool> {
    document
        .component_state::<State>(disclosure)
        .open_read
        .clone()
}

pub fn disclosure_hovered(document: &Document, disclosure: NodeId) -> ReadSignal<bool> {
    let button = document.component_state::<State>(disclosure).button;
    unstyled::button_hovered(document, button)
}

pub fn disclosure_focused(document: &Document, disclosure: NodeId) -> ReadSignal<bool> {
    let button = document.component_state::<State>(disclosure).button;
    unstyled::button_focused(document, button)
}

pub fn set_disclosure_on_toggle(
    disclosure: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    with_document(|document| {
        document.component_state_mut::<State>(disclosure).on_toggle = Some(Box::new(handler));
    });
}

pub fn focus_disclosure(disclosure: NodeId) {
    with_document(|document| {
        let button = document.component_state::<State>(disclosure).button;
        unstyled::focus_button(button);
    });
}
