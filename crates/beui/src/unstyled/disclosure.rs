use beui_macros::{component, view};

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    component_detail, set_component_state, untrack, Callback, Children, ColumnBuilder, Prop,
    ReadSignal, Render, VisibilityBuilder,
};
use crate::unstyled;
use crate::unstyled::button::ButtonHandle;

pub struct DisclosureHandle {
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub open: ReadSignal<bool>,
}

struct State {
    open: ReadSignal<bool>,
}

#[component]
pub fn disclosure(
    spacing: Prop<f32>,
    header: Render<DisclosureHandle>,
    children: Children,
    open: Prop<bool>,
    on_toggle: Callback<bool>,
) -> NodeId {
    let content = children
        .into_first()
        .expect("disclosure requires content, e.g. <unstyled::disclosure>{intrinsic(node)}</unstyled::disclosure>");

    let (open_read, set_open) = open.signal();

    component_detail(open_read.map(|open| if open { "open" } else { "closed" }.to_owned()));

    set_component_state(State {
        open: open_read.clone(),
    });

    let open_for_header = open_read.clone();
    let open_for_click = open_read.clone();

    view! {
        <column spacing={spacing}>
            <unstyled::button
                on_click={move || {
                    let next = !untrack(|| open_for_click.get());
                    set_open.set(next);
                    on_toggle.call(next);
                }}
                content={move |handle: ButtonHandle| {
                    header.call(DisclosureHandle {
                        hovered: handle.hovered,
                        active: handle.active,
                        focused: handle.focused,
                        open: open_for_header,
                    })
                }}
            />
            <visibility visible={open_read}>{content}</visibility>
        </column>
    }
}

pub fn disclosure_open(document: &Document, disclosure: NodeId) -> bool {
    document.component_state::<State>(disclosure).open.get()
}
