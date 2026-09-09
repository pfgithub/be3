use beui_macros::component;

use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{create_effect, with_document, Prop};
use crate::styled::theme::{
    ACCENT_SOFT, BORDER, BORDER_WIDTH, FONT_BODY, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::unstyled;
use crate::unstyled::MenuItem;

const PADDING_HORIZONTAL: f32 = 14.0;
const PADDING_VERTICAL: f32 = 6.0;
const MENU_PADDING: f32 = 4.0;
const MENU_WIDTH: f32 = 200.0;

#[component]
pub fn context_menu(
    region: NodeId,
    items: Prop<Vec<MenuItem>>,
    on_select: Option<Handler<Vec<usize>>>,
) -> NodeId {
    let mut on_select = on_select;
    let inner = with_document(|document| unstyled::context_menu(document, region, Vec::new()));

    with_document(|document| {
        unstyled::set_context_menu_on_select(document, inner, move |document, path| {
            if let Some(handler) = &mut on_select {
                handler(document, path);
            }
        });
    });

    items.apply(move |items| {
        with_document(|document| {
            let styling_items = items.clone();
            unstyled::set_context_menu_items(document, inner, items);
            let overlay = unstyled::context_menu_overlay(document, inner);
            style_menu_panel(document, overlay, &styling_items);
        });
    });

    inner
}

fn style_menu_panel(document: &mut Document, overlay: NodeId, items: &[MenuItem]) {
    let menu = document
        .overlay_content(overlay)
        .expect("menu overlay always has content");
    style_menu_rows(document, menu, items);

    let padding = document.create_padding(MENU_PADDING, MENU_PADDING);
    document.set_padding_child(padding, menu);
    let fill = document.create_fill(SURFACE_RAISED, RADIUS);
    document.set_fill_child(fill, padding);
    let border = document.create_outline(BORDER, BORDER_WIDTH, RADIUS, 0.0);
    document.set_outline_visible(border, true);
    document.set_outline_child(border, fill);
    let sized = document.create_sized(Some(MENU_WIDTH), None);
    document.set_sized_child(sized, border);
    document.set_overlay_content(overlay, sized);
}

fn style_menu_rows(document: &mut Document, menu: NodeId, items: &[MenuItem]) {
    for (index, item) in items.iter().enumerate() {
        let button = unstyled::menu_list_row_button(document, menu, index);
        let color = if item.disabled { TEXT_MUTED } else { TEXT };
        let label = document.create_text(item.label.clone(), FONT_BODY, color);
        document.set_text_align(label, TextAlign::Start, TextAlign::Center);

        let padding = document.create_padding(PADDING_HORIZONTAL, PADDING_VERTICAL);
        document.set_padding_child(padding, label);
        let fill = document.create_fill(Color32::TRANSPARENT, RADIUS);
        document.set_fill_child(fill, padding);
        unstyled::set_button_child(button, fill);

        let hovered = unstyled::button_hovered(document, button);
        let focused = unstyled::button_focused(document, button);
        create_effect(move || {
            let color = row_background(focused.get(), hovered.get());
            with_document(|document| document.set_fill_color(fill, color));
        });

        if !item.children.is_empty() {
            if let Some(overlay) = unstyled::menu_list_row_submenu_overlay(document, menu, index) {
                style_menu_panel(document, overlay, &item.children);
            }
        }
    }
}

fn row_background(focused: bool, hovered: bool) -> Color32 {
    match (focused, hovered) {
        (true, _) => ACCENT_SOFT,
        (false, true) => BORDER,
        (false, false) => Color32::TRANSPARENT,
    }
}
