use crate::unstyled;

use crate::base::{Direction, ItemSize};
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{create_signal, with_document, ReadSignal, WriteSignal};
use beui_macros::view;

struct State {
    visibility: NodeId,
    button: NodeId,
    open: bool,
    open_read: ReadSignal<bool>,
    open_write: WriteSignal<bool>,
    on_toggle: Option<Handler<bool>>,
}

pub fn disclosure(spacing: f32, open: bool) -> NodeId {
    with_document(|document| {
        let header = document.create_slot("header");
        let button = view! { <unstyled::button /> };
        unstyled::set_button_child(button, header);

        let content = document.create_slot("content");
        let visibility = document.create_visibility(open);
        document.set_visibility_child(visibility, content);

        let column = document.create_list(Direction::Vertical, spacing);
        document.append_child(column, button, ItemSize::Intrinsic);
        document.append_child(column, visibility, ItemSize::Intrinsic);

        let disclosure = document.create_shadow("disclosure", column, vec![header, content]);
        document.set_component_detail(disclosure, detail(open));
        let (open_read, open_write) = create_signal(open);
        document.set_component_state(
            disclosure,
            State {
                visibility,
                button,
                open,
                open_read,
                open_write,
                on_toggle: None,
            },
        );

        unstyled::set_button_on_click(button, move |document| {
            let open = disclosure_open(document, disclosure);
            set_disclosure_open(document, disclosure, !open);
        });

        disclosure
    })
}

pub fn set_disclosure_header(disclosure: NodeId, child: NodeId) {
    with_document(|document| {
        let slot = document.shadow_slots(disclosure)[0];
        document.set_slot_child(slot, child);
    });
}

pub fn set_disclosure_content(disclosure: NodeId, child: NodeId) {
    with_document(|document| {
        let slot = document.shadow_slots(disclosure)[1];
        document.set_slot_child(slot, child);
    });
}

pub fn disclosure_open(document: &Document, disclosure: NodeId) -> bool {
    document.component_state::<State>(disclosure).open
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

pub fn set_disclosure_open(document: &mut Document, disclosure: NodeId, open: bool) {
    let state = document.component_state_mut::<State>(disclosure);
    if state.open == open {
        return;
    }
    state.open = open;
    state.open_write.set(open);
    let visibility = state.visibility;
    document.set_visible(visibility, open);
    document.set_component_detail(disclosure, detail(open));
    document.call_component_handler(disclosure, open, |state: &mut State| &mut state.on_toggle);
}

fn detail(open: bool) -> &'static str {
    if open {
        "open"
    } else {
        "closed"
    }
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
