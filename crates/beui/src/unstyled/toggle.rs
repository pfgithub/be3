use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::NodeId;

pub fn toggle(document: &mut Document, checked: bool) -> NodeId {
    let slot = document.create_slot("content");
    let checkable = document.create_checkable(checked);
    document.set_checkable_child(checkable, slot);

    let click_catcher = document.create_click_catcher(CursorIcon::PointingHand);
    document.set_click_catcher_child(click_catcher, checkable);
    document.set_click_catcher_on_click(click_catcher, move |document| {
        let checked = document.is_checked(checkable);
        document.set_checked(checkable, !checked);
    });

    let focusable = document.create_focusable();
    document.set_focusable_child(focusable, click_catcher);
    document.set_focusable_on_activate_change(focusable, move |document, pressed| {
        document.set_click_catcher_key_active(click_catcher, pressed);
    });
    document.set_focusable_on_activate(focusable, move |document| {
        document.click_click_catcher(click_catcher);
    });

    document.create_shadow("toggle", focusable, vec![slot])
}

pub fn set_toggle_child(document: &mut Document, toggle: NodeId, child: NodeId) {
    document.set_shadow_child(toggle, child);
}

pub fn toggle_checked(document: &Document, toggle: NodeId) -> bool {
    document.is_checked(checkable(document, toggle))
}

pub fn set_toggle_checked(document: &mut Document, toggle: NodeId, checked: bool) {
    let checkable = checkable(document, toggle);
    document.set_checked(checkable, checked);
}

pub fn add_toggle_on_change(
    document: &mut Document,
    toggle: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    let checkable = checkable(document, toggle);
    document.add_checkable_on_change(checkable, handler);
}

pub fn set_toggle_on_hover_change(
    document: &mut Document,
    toggle: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    let click_catcher = click_catcher(document, toggle);
    document.set_click_catcher_on_hover_change(click_catcher, handler);
}

pub fn set_toggle_on_active_change(
    document: &mut Document,
    toggle: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    let click_catcher = click_catcher(document, toggle);
    document.set_click_catcher_on_active_change(click_catcher, handler);
}

pub fn set_toggle_on_focus_change(
    document: &mut Document,
    toggle: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    let focusable = document.shadow_root(toggle);
    document.set_focusable_on_focus_change(focusable, handler);
}

pub fn focus_toggle(document: &mut Document, toggle: NodeId) {
    let focusable = document.shadow_root(toggle);
    document.focus_focusable(focusable);
}

fn click_catcher(document: &Document, toggle: NodeId) -> NodeId {
    let focusable = document.shadow_root(toggle);
    document
        .focusable_child(focusable)
        .expect("toggle is missing its click catcher")
}

fn checkable(document: &Document, toggle: NodeId) -> NodeId {
    let click_catcher = click_catcher(document, toggle);
    document
        .click_catcher_child(click_catcher)
        .expect("toggle is missing its checkable")
}
