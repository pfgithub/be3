use std::rc::Rc;

use crate::color::Color32;
use crate::input::CursorIcon;

use crate::base::{ItemSize, TextAlign};
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{create_signal, view, with_reactive_scope, WriteSignal};
use crate::styled;
use crate::styled::theme::{
    ACCENT, BORDER_WIDTH, CHIP_RADIUS, ON_ACCENT, RADIUS, SCROLLBAR_WIDTH, SEPARATOR_HEIGHT,
    SURFACE, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::styled::{CaptionBuilder, HeadingBuilder, ListRowBuilder};
use crate::unstyled;

use super::tree::Entry;
use super::State;

const HEADER_PADDING: f32 = 12.0;
const HEADER_SPACING: f32 = 8.0;
const BODY_PADDING: f32 = 8.0;
const BODY_SPACING: f32 = 6.0;
const FOOTER_PADDING: f32 = 10.0;
const FOOTER_SPACING: f32 = 3.0;
const ROW_SPACING: f32 = 6.0;
const INDENT: f32 = 12.0;
const MARKER_WIDTH: f32 = 14.0;
const TOGGLE_PADDING_HORIZONTAL: f32 = 8.0;
const TOGGLE_PADDING_VERTICAL: f32 = 3.0;

pub(crate) struct Summary {
    pub(crate) total: usize,
    pub(crate) picking: bool,
    pub(crate) selection: String,
    pub(crate) bounds: String,
}

pub(crate) struct Row {
    pub(crate) row: NodeId,
    #[cfg(test)]
    pub(crate) marker: NodeId,
    pub(crate) outline: NodeId,
    pub(crate) detail: NodeId,
    pub(crate) size: NodeId,
}

pub(crate) struct Panel {
    pub(crate) document: Document,
    pub(crate) scroll: NodeId,
    pub(crate) set_count: WriteSignal<String>,
    pub(crate) toggle: NodeId,
    pub(crate) toggle_label: NodeId,
    pub(crate) selection: NodeId,
    pub(crate) bounds: NodeId,
    pub(crate) rows: Vec<Row>,
}

pub(crate) fn build(entries: &[Entry], summary: &Summary, state: &Rc<State>, offset: f32) -> Panel {
    let mut document = Document::new();
    document.inspectable = false;

    let (count, set_count) = count_label(&mut document, summary.total);
    let (picker, fill, label) = pick_toggle(&mut document, state, summary.picking);
    let header = header(&mut document, count, picker);

    let scroll = document.create_scroll();
    let mut rows = Vec::new();
    for entry in entries {
        let row = row(&mut document, entry, state);
        document.append_scroll_item(scroll, row.row);
        rows.push(row);
    }
    document.set_scroll_offset(scroll, offset);
    let body = body(&mut document, scroll);

    let selection = styled::code(&mut document, summary.selection.clone());
    let bounds = styled::code(&mut document, summary.bounds.clone());
    document.set_text_color(bounds, TEXT_MUTED);
    let footer = footer(&mut document, selection, bounds);

    let column = unstyled::column(&mut document, 0.0);
    document.append_child(column, header, ItemSize::Intrinsic);
    let above = styled::separator(&mut document);
    document.append_child(column, above, ItemSize::Fixed(SEPARATOR_HEIGHT));
    document.append_child(column, body, ItemSize::Percent(100.0));
    let below = styled::separator(&mut document);
    document.append_child(column, below, ItemSize::Fixed(SEPARATOR_HEIGHT));
    document.append_child(column, footer, ItemSize::Intrinsic);

    let surface = document.create_fill(SURFACE, 0);
    document.set_fill_child(surface, column);

    let edge = styled::separator(&mut document);
    let panel = unstyled::row(&mut document, 0.0);
    document.append_child(panel, edge, ItemSize::Fixed(SEPARATOR_HEIGHT));
    document.append_child(panel, surface, ItemSize::Percent(100.0));
    document.set_root(panel);

    Panel {
        document,
        scroll,
        set_count,
        toggle: fill,
        toggle_label: label,
        selection,
        bounds,
        rows,
    }
}

pub(crate) fn total_label(total: usize) -> String {
    match total {
        1 => "1 node".to_owned(),
        total => format!("{total} nodes"),
    }
}

pub(crate) fn toggle_fill(picking: bool) -> Color32 {
    if picking {
        ACCENT
    } else {
        SURFACE_RAISED
    }
}

pub(crate) fn toggle_text(picking: bool) -> Color32 {
    if picking {
        ON_ACCENT
    } else {
        TEXT
    }
}

fn count_label(document: &mut Document, total: usize) -> (NodeId, WriteSignal<String>) {
    let (count_text, set_count_text) = create_signal(total_label(total));
    let count = with_reactive_scope(document, || view! { <caption content={count_text} /> });
    let count_text_node = document.shadow_root(count);
    document.set_text_align(count_text_node, TextAlign::End, TextAlign::Center);
    (count, set_count_text)
}

fn header(document: &mut Document, count: NodeId, toggle: NodeId) -> NodeId {
    let title = with_reactive_scope(
        document,
        || view! { <heading content={"Inspector".to_string()} /> },
    );
    let line = unstyled::centered_row(document, HEADER_SPACING);
    document.append_child(line, title, ItemSize::Intrinsic);
    document.append_child(line, count, ItemSize::Percent(100.0));
    document.append_child(line, toggle, ItemSize::Intrinsic);
    let padding = document.create_padding(HEADER_PADDING, HEADER_PADDING);
    document.set_padding_child(padding, line);
    padding
}

fn pick_toggle(
    document: &mut Document,
    state: &Rc<State>,
    picking: bool,
) -> (NodeId, NodeId, NodeId) {
    let label = styled::code(document, "Pick");
    document.set_text_align(label, TextAlign::Center, TextAlign::Center);
    document.set_text_color(label, toggle_text(picking));

    let padding = document.create_padding(TOGGLE_PADDING_HORIZONTAL, TOGGLE_PADDING_VERTICAL);
    document.set_padding_child(padding, label);

    let fill = document.create_fill(toggle_fill(picking), CHIP_RADIUS);
    document.set_fill_child(fill, padding);

    let bordered = styled::bordered(document, fill, CHIP_RADIUS);
    let pressable = unstyled::pressable(document);
    unstyled::set_pressable_child(document, pressable, bordered);

    let picker = state.clone();
    unstyled::set_pressable_on_click(document, pressable, move |_document| {
        picker.toggle_picking()
    });

    (pressable, fill, label)
}

fn body(document: &mut Document, scroll: NodeId) -> NodeId {
    let bar = styled::scrollbar(document, scroll);
    let area = unstyled::row(document, BODY_SPACING);
    document.append_child(area, scroll, ItemSize::Percent(100.0));
    document.append_child(area, bar, ItemSize::Fixed(SCROLLBAR_WIDTH));
    let padding = document.create_padding(BODY_PADDING, BODY_PADDING);
    document.set_padding_child(padding, area);
    padding
}

fn footer(document: &mut Document, selection: NodeId, bounds: NodeId) -> NodeId {
    let column = unstyled::column(document, FOOTER_SPACING);
    document.append_child(column, selection, ItemSize::Intrinsic);
    document.append_child(column, bounds, ItemSize::Intrinsic);
    let padding = document.create_padding(FOOTER_PADDING, FOOTER_PADDING);
    document.set_padding_child(padding, column);
    padding
}

fn row(document: &mut Document, entry: &Entry, state: &Rc<State>) -> Row {
    let indent = unstyled::spacer(document);
    let marker = marker(document, entry, state);

    let kind = styled::code(document, entry.kind);

    let detail = styled::code(document, entry.detail.clone());
    document.set_text_color(detail, TEXT_MUTED);

    let size = styled::code(document, entry.size.clone());
    document.set_text_color(size, TEXT_MUTED);
    document.set_text_align(size, TextAlign::End, TextAlign::Center);

    let line = unstyled::centered_row(document, ROW_SPACING);
    document.append_child(line, indent, ItemSize::Fixed(entry.depth as f32 * INDENT));
    document.append_child(line, marker, ItemSize::Fixed(MARKER_WIDTH));
    document.append_child(line, kind, ItemSize::Intrinsic);
    document.append_child(line, detail, ItemSize::Percent(100.0));
    document.append_child(line, size, ItemSize::Intrinsic);

    let list_row = with_reactive_scope(document, || view! { <list_row>{line}</list_row> });
    let outline = document.create_outline(ACCENT, BORDER_WIDTH, RADIUS, 0.0);
    document.set_outline_visible(outline, entry.selected);
    document.set_outline_child(outline, list_row);

    let row = document.create_click_catcher(CursorIcon::PointingHand);
    document.set_click_catcher_child(row, outline);

    let node = entry.key.node();
    let hover = state.clone();
    document.set_click_catcher_on_hover_change(row, move |_document, hovered| {
        hover.hover(node, hovered);
    });
    let selection = state.clone();
    document.set_click_catcher_on_click(row, move |_document| selection.select(node));

    Row {
        row,
        #[cfg(test)]
        marker,
        outline,
        detail,
        size,
    }
}

fn marker(document: &mut Document, entry: &Entry, state: &Rc<State>) -> NodeId {
    let glyph = styled::code(document, glyph(entry));
    document.set_text_color(glyph, TEXT_MUTED);
    document.set_text_align(glyph, TextAlign::Center, TextAlign::Center);
    if !entry.expandable {
        return glyph;
    }

    let marker = unstyled::pressable(document);
    unstyled::set_pressable_child(document, marker, glyph);

    let expansion = state.clone();
    let key = entry.key;
    let expanded = entry.expanded;
    unstyled::set_pressable_on_click(document, marker, move |_document| {
        expansion.set_expanded(key, !expanded);
    });
    marker
}

fn glyph(entry: &Entry) -> &'static str {
    match (entry.expandable, entry.expanded) {
        (true, true) => "-",
        (true, false) => "+",
        (false, _) => "",
    }
}
