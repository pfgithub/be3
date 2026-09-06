use crate::input::CursorIcon;

use crate::base::{Direction, ItemSize};
use crate::document::Document;
use crate::node::{Handler, NodeId};

struct State {
    visibility: NodeId,
    open: bool,
    hovered: bool,
    on_toggle: Option<Handler<bool>>,
    on_hover_change: Option<Handler<bool>>,
}

pub fn disclosure(document: &mut Document, spacing: f32, open: bool) -> NodeId {
    let header = document.create_slot("header");
    let click_catcher = document.create_click_catcher(CursorIcon::PointingHand);
    document.set_click_catcher_child(click_catcher, header);

    let content = document.create_slot("content");
    let visibility = document.create_visibility(open);
    document.set_visibility_child(visibility, content);

    let column = document.create_list(Direction::Vertical, spacing);
    document.append_child(column, click_catcher, ItemSize::Intrinsic);
    document.append_child(column, visibility, ItemSize::Intrinsic);

    let disclosure = document.create_shadow("disclosure", column, vec![header, content]);
    document.set_component_detail(disclosure, detail(open));
    document.set_component_state(
        disclosure,
        State {
            visibility,
            open,
            hovered: false,
            on_toggle: None,
            on_hover_change: None,
        },
    );

    document.set_click_catcher_on_click(click_catcher, move |document| {
        let open = disclosure_open(document, disclosure);
        set_disclosure_open(document, disclosure, !open);
    });
    document.set_click_catcher_on_hover_change(click_catcher, move |document, hovered| {
        document.component_state_mut::<State>(disclosure).hovered = hovered;
        document.call_component_handler(disclosure, hovered, |state: &mut State| {
            &mut state.on_hover_change
        });
    });

    disclosure
}

pub fn set_disclosure_header(document: &mut Document, disclosure: NodeId, child: NodeId) {
    let slot = document.shadow_slots(disclosure)[0];
    document.set_slot_child(slot, child);
}

pub fn set_disclosure_content(document: &mut Document, disclosure: NodeId, child: NodeId) {
    let slot = document.shadow_slots(disclosure)[1];
    document.set_slot_child(slot, child);
}

pub fn disclosure_open(document: &Document, disclosure: NodeId) -> bool {
    document.component_state::<State>(disclosure).open
}

pub fn disclosure_hovered(document: &Document, disclosure: NodeId) -> bool {
    document.component_state::<State>(disclosure).hovered
}

pub fn set_disclosure_open(document: &mut Document, disclosure: NodeId, open: bool) {
    let state = document.component_state_mut::<State>(disclosure);
    if state.open == open {
        return;
    }
    state.open = open;
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
    document: &mut Document,
    disclosure: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document.component_state_mut::<State>(disclosure).on_toggle = Some(Box::new(handler));
}

pub fn set_disclosure_on_hover_change(
    document: &mut Document,
    disclosure: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    document
        .component_state_mut::<State>(disclosure)
        .on_hover_change = Some(Box::new(handler));
}
