use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{
    with_document, FillBuilder, OutlineBuilder, PaddingBuilder, Prop, SizedBuilder, TextBuilder,
};
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
    let inner = unstyled::ContextMenuBuilder::default()
        .region(region)
        .items(Vec::new())
        .build();

    unstyled::set_context_menu_on_select(inner, move |document, path| {
        if let Some(handler) = &mut on_select {
            handler(document, path);
        }
    });

    items.apply(move |items| {
        let styling_items = items.clone();
        unstyled::set_context_menu_items(inner, items);
        with_document(|document| {
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

    let sized = view! {
        <sized width={MENU_WIDTH}>
            <outline color={BORDER} width={BORDER_WIDTH} radius={RADIUS} offset={0.0} visible={true}>
                <fill color={SURFACE_RAISED} radius={RADIUS}>
                    <padding horizontal={MENU_PADDING} vertical={MENU_PADDING}>{menu}</padding>
                </fill>
            </outline>
        </sized>
    };
    document.set_overlay_content(overlay, sized);
}

fn style_menu_rows(document: &mut Document, menu: NodeId, items: &[MenuItem]) {
    for (index, item) in items.iter().enumerate() {
        let button = unstyled::menu_list_row_button(document, menu, index);
        let color = if item.disabled { TEXT_MUTED } else { TEXT };

        let hovered = unstyled::button_hovered(document, button);
        let focused = unstyled::button_focused(document, button);
        let fill_color = Prop::Dynamic(Box::new(move || {
            row_background(focused.get(), hovered.get())
        }));
        let fill = view! {
            <fill color={fill_color} radius={RADIUS}>
                <padding horizontal={PADDING_HORIZONTAL} vertical={PADDING_VERTICAL}>
                    <text string={item.label.clone()} font_size={FONT_BODY} color={color} align={TextAlign::Start} />
                </padding>
            </fill>
        };
        unstyled::set_button_child(button, fill);

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
