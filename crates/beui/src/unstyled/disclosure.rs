use crate::input::CursorIcon;

use crate::base::{Direction, ItemSize};
use crate::document::Document;
use crate::node::NodeId;

pub fn disclosure(document: &mut Document, spacing: f32, open: bool) -> NodeId {
    let header = document.create_slot("header");
    let click_catcher = document.create_click_catcher(CursorIcon::PointingHand);
    document.set_click_catcher_child(click_catcher, header);

    let content = document.create_slot("content");
    let visibility = document.create_visibility(open);
    document.set_visibility_child(visibility, content);

    document.set_click_catcher_on_click(click_catcher, move |document| {
        let open = document.is_visible(visibility);
        document.set_visible(visibility, !open);
    });

    let column = document.create_list(Direction::Vertical, spacing);
    document.append_child(column, click_catcher, ItemSize::Intrinsic);
    document.append_child(column, visibility, ItemSize::Intrinsic);

    document.create_shadow("disclosure", column, vec![header, content])
}

pub fn set_disclosure_header(document: &mut Document, disclosure: NodeId, child: NodeId) {
    let slot = slots(document, disclosure)[0];
    document.set_slot_child(slot, child);
}

pub fn set_disclosure_content(document: &mut Document, disclosure: NodeId, child: NodeId) {
    let slot = slots(document, disclosure)[1];
    document.set_slot_child(slot, child);
}

pub fn disclosure_open(document: &Document, disclosure: NodeId) -> bool {
    document.is_visible(visibility(document, disclosure))
}

pub fn set_disclosure_open(document: &mut Document, disclosure: NodeId, open: bool) {
    let visibility = visibility(document, disclosure);
    document.set_visible(visibility, open);
}

pub fn add_disclosure_on_toggle(
    document: &mut Document,
    disclosure: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    let visibility = visibility(document, disclosure);
    document.add_visibility_on_change(visibility, handler);
}

pub fn set_disclosure_on_hover_change(
    document: &mut Document,
    disclosure: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    let click_catcher = click_catcher(document, disclosure);
    document.set_click_catcher_on_hover_change(click_catcher, handler);
}

fn slots(document: &Document, disclosure: NodeId) -> Vec<NodeId> {
    document.shadow_slots(disclosure)
}

fn click_catcher(document: &Document, disclosure: NodeId) -> NodeId {
    document.children(document.shadow_root(disclosure))[0]
}

fn visibility(document: &Document, disclosure: NodeId) -> NodeId {
    document.children(document.shadow_root(disclosure))[1]
}
