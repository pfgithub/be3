use beui_macros::{component, view};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    component_detail, create_signal, current_component, set_component_state, untrack,
    with_document, Callback, Children, ColumnBuilder, Prop, ReadSignal, VisibilityBuilder,
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
    let disclosure = current_component();
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

    let button_cell = std::cell::Cell::new(None);

    let root = view! {
        <column spacing={spacing}>
            {{
                let button = view! {
                    <unstyled::button
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
                        })} />
                };
                button_cell.set(Some(button));
                button
            }}
            <visibility visible={open_read.clone()}>{content}</visibility>
        </column>
    };

    with_document(|document| {
        set_component_state(
            document,
            disclosure,
            State {
                button: button_cell.get().expect("disclosure button not yet built"),
                open: open_read,
            },
        );
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
    let button = document.component_state::<State>(disclosure).button;
    unstyled::button_hovered(document, button)
}

pub fn disclosure_focused(document: &Document, disclosure: NodeId) -> ReadSignal<bool> {
    let button = document.component_state::<State>(disclosure).button;
    unstyled::button_focused(document, button)
}

pub fn focus_disclosure(disclosure: NodeId) {
    with_document(|document| {
        let button = document.component_state::<State>(disclosure).button;
        unstyled::focus_button(button);
    });
}
