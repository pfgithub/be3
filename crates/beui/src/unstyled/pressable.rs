use egui::CursorIcon;

use crate::document::Document;
use crate::node::NodeId;

pub fn pressable(document: &mut Document) -> NodeId {
    let click_catcher = document.create_click_catcher(CursorIcon::PointingHand);
    let slot = document.create_slot();
    document.set_click_catcher_child(click_catcher, slot);
    document.create_shadow(click_catcher, slot)
}

pub fn set_pressable_child(document: &mut Document, pressable: NodeId, child: NodeId) {
    document.set_shadow_child(pressable, child);
}

pub fn set_pressable_on_click(
    document: &mut Document,
    pressable: NodeId,
    handler: impl FnMut(&mut Document) + 'static,
) {
    let click_catcher = document.shadow_root(pressable);
    document.set_click_catcher_on_click(click_catcher, handler);
}

pub fn set_pressable_on_hover_change(
    document: &mut Document,
    pressable: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    let click_catcher = document.shadow_root(pressable);
    document.set_click_catcher_on_hover_change(click_catcher, handler);
}

pub fn set_pressable_on_active_change(
    document: &mut Document,
    pressable: NodeId,
    handler: impl FnMut(&mut Document, bool) + 'static,
) {
    let click_catcher = document.shadow_root(pressable);
    document.set_click_catcher_on_active_change(click_catcher, handler);
}
