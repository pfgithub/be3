use beui_macros::component;

use crate::color::Color32;

use crate::base::ItemSize;
use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{current_component, set_component_detail, with_document, Prop};
use crate::styled::text::body_line;
use crate::styled::theme::{
    ACCENT, ACCENT_HOVER, BORDER, BORDER_WIDTH, CHIP_RADIUS, ON_ACCENT, RADIUS, SURFACE_RAISED,
};
use crate::unstyled;

const BOX_SIZE: f32 = 18.0;
const MARK_SIZE: f32 = 10.0;
const MARK_RADIUS: u8 = 2;
const SPACING: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 4.0;

#[component]
pub fn checkbox(
    label: Prop<String>,
    checked: Prop<bool>,
    on_change: Option<Handler<bool>>,
) -> NodeId {
    let shadow = current_component();
    let mut on_change = on_change;

    let (toggle, mark_visibility, fill, border, ring, label_node) = with_document(|document| {
        let toggle = unstyled::toggle(document, false);

        let mark = document.create_fill(ON_ACCENT, MARK_RADIUS);
        let mark_size = document.create_sized(Some(MARK_SIZE), Some(MARK_SIZE));
        document.set_sized_child(mark_size, mark);
        let mark_visibility = document.create_visibility(false);
        document.set_visibility_child(mark_visibility, mark_size);

        let before = unstyled::spacer(document);
        let after = unstyled::spacer(document);
        let center = unstyled::centered_row(document, 0.0);
        document.append_child(center, before, ItemSize::Percent(100.0));
        document.append_child(center, mark_visibility, ItemSize::Intrinsic);
        document.append_child(center, after, ItemSize::Percent(100.0));

        let fill = document.create_fill(box_fill(false, false), CHIP_RADIUS);
        document.set_fill_child(fill, center);

        let border = document.create_outline(BORDER, BORDER_WIDTH, CHIP_RADIUS, 0.0);
        document.set_outline_visible(border, true);
        document.set_outline_child(border, fill);

        let boxed = document.create_sized(Some(BOX_SIZE), Some(BOX_SIZE));
        document.set_sized_child(boxed, border);

        let label_node = body_line(document, String::new());
        let line = unstyled::centered_row(document, SPACING);
        document.append_child(line, boxed, ItemSize::Intrinsic);
        document.append_child(line, label_node, ItemSize::Percent(100.0));

        let ring = document.create_outline(ACCENT, FOCUS_RING_WIDTH, RADIUS, FOCUS_RING_OFFSET);
        document.set_outline_child(ring, line);
        unstyled::set_toggle_child(document, toggle, ring);

        (toggle, mark_visibility, fill, border, ring, label_node)
    });

    label.apply(move |value| {
        with_document(|document| document.set_text(label_node, value));
    });

    with_document(|document| {
        unstyled::set_toggle_on_change(document, toggle, move |document, checked| {
            let hovered = unstyled::toggle_hovered(document, toggle);
            document.set_visible(mark_visibility, checked);
            document.set_fill_color(fill, box_fill(checked, hovered));
            document.set_outline_visible(border, !checked);
            set_component_detail(document, shadow, detail(checked));
            if let Some(handler) = &mut on_change {
                handler(document, checked);
            }
        });
        unstyled::set_toggle_on_hover_change(document, toggle, move |document, hovered| {
            let checked = unstyled::toggle_checked(document, toggle);
            document.set_fill_color(fill, box_fill(checked, hovered));
        });
        unstyled::set_toggle_on_focus_change(document, toggle, move |document, focused| {
            document.set_outline_visible(ring, focused);
        });

        set_component_detail(document, shadow, detail(false));
    });

    checked.apply(move |checked| {
        with_document(|document| unstyled::set_toggle_checked(document, toggle, checked));
    });

    toggle
}

pub fn checkbox_checked(document: &Document, checkbox: NodeId) -> bool {
    unstyled::toggle_checked(document, document.shadow_root(checkbox))
}

fn detail(checked: bool) -> &'static str {
    if checked {
        "checked"
    } else {
        "unchecked"
    }
}

fn box_fill(checked: bool, hovered: bool) -> Color32 {
    match (checked, hovered) {
        (true, false) => ACCENT,
        (true, true) => ACCENT_HOVER,
        (false, false) => SURFACE_RAISED,
        (false, true) => BORDER,
    }
}
