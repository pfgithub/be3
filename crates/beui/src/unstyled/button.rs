use crate::input::CursorIcon;

use crate::document::Document;
use crate::node::NodeId;

pub fn button(document: &mut Document) -> NodeId {
    let click_catcher = document.create_click_catcher(CursorIcon::PointingHand);
    let focusable = document.create_focusable();
    document.set_focusable_child(focusable, click_catcher);
    let slot = document.create_slot("content");
    document.set_click_catcher_child(click_catcher, slot);
    document.set_focusable_on_activate_change(focusable, move |document, pressed| {
        document.set_click_catcher_key_active(click_catcher, pressed);
    });
    document.set_focusable_on_activate(focusable, move |document| {
        document.click_click_catcher(click_catcher);
    });
    document.create_shadow("button", focusable, vec![slot])
}

pub fn set_button_child(document: &mut Document, button: NodeId, child: NodeId) {
    document.set_shadow_child(button, child);
}

pub fn set_button_on_click(
    document: &mut Document,
    button: NodeId,
    handler: impl FnMut(&mut Document) + 'static,
) {
    let click_catcher = click_catcher(document, button);
    document.set_click_catcher_on_click(click_catcher, handler);
}

pub fn set_button_on_hover_change(
    document: &mut Document,
    button: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    let click_catcher = click_catcher(document, button);
    document.set_click_catcher_on_hover_change(click_catcher, handler);
}

pub fn set_button_on_active_change(
    document: &mut Document,
    button: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    let click_catcher = click_catcher(document, button);
    document.set_click_catcher_on_active_change(click_catcher, handler);
}

pub fn set_button_on_focus_change(
    document: &mut Document,
    button: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    let focusable = document.shadow_root(button);
    document.set_focusable_on_focus_change(focusable, handler);
}

pub fn focus_button(document: &mut Document, button: NodeId) {
    let focusable = document.shadow_root(button);
    document.focus_focusable(focusable);
}

fn click_catcher(document: &Document, button: NodeId) -> NodeId {
    let focusable = document.shadow_root(button);
    document
        .focusable_child(focusable)
        .expect("button is missing its click catcher")
}
