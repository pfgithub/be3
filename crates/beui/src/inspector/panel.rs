use std::rc::Rc;

use crate::color::Color32;
use crate::input::CursorIcon;

use crate::base::{ItemSize, TextAlign};
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{create_signal, view, with_reactive_scope, ClickCatcherBuilder, WriteSignal};
use crate::styled::theme::{
    ACCENT, BORDER_WIDTH, CHIP_RADIUS, ON_ACCENT, RADIUS, SCROLLBAR_WIDTH, SEPARATOR_HEIGHT,
    SURFACE, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::styled::{
    BorderedBuilder, CaptionBuilder, CodeBuilder, HeadingBuilder, ListRowBuilder, ScrollbarBuilder,
    SeparatorBuilder,
};
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

    let selection = with_reactive_scope(
        &mut document,
        || view! { <code content={summary.selection.clone()} /> },
    );
    let selection = document.shadow_root(selection);
    let bounds = with_reactive_scope(
        &mut document,
        || view! { <code content={summary.bounds.clone()} /> },
    );
    let bounds = document.shadow_root(bounds);
    document.set_text_color(bounds, TEXT_MUTED);
    let footer = footer(&mut document, selection, bounds);

    let column = unstyled::column(&mut document, 0.0);
    document.append_child(column, header, ItemSize::Intrinsic);
    let above = with_reactive_scope(&mut document, || view! { <separator /> });
    document.append_child(column, above, ItemSize::Fixed(SEPARATOR_HEIGHT));
    document.append_child(column, body, ItemSize::Percent(100.0));
    let below = with_reactive_scope(&mut document, || view! { <separator /> });
    document.append_child(column, below, ItemSize::Fixed(SEPARATOR_HEIGHT));
    document.append_child(column, footer, ItemSize::Intrinsic);

    let surface = document.create_fill(SURFACE, 0);
    document.set_fill_child(surface, column);

    let edge = with_reactive_scope(&mut document, || view! { <separator /> });
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
    let label = with_reactive_scope(document, || view! { <code content={"Pick".to_owned()} /> });
    let label = document.shadow_root(label);
    document.set_text_align(label, TextAlign::Center, TextAlign::Center);
    document.set_text_color(label, toggle_text(picking));

    let padding = document.create_padding(TOGGLE_PADDING_HORIZONTAL, TOGGLE_PADDING_VERTICAL);
    document.set_padding_child(padding, label);

    let fill = document.create_fill(toggle_fill(picking), CHIP_RADIUS);
    document.set_fill_child(fill, padding);

    let bordered = with_reactive_scope(document, || {
        view! { <bordered corner_radius={CHIP_RADIUS}>{fill}</bordered> }
    });
    let pressable = with_reactive_scope(document, || unstyled::PressableBuilder::default().build());
    unstyled::set_pressable_child(document, pressable, bordered);

    let picker = state.clone();
    unstyled::set_pressable_on_click(document, pressable, move |_document| {
        picker.toggle_picking()
    });

    (pressable, fill, label)
}

fn body(document: &mut Document, scroll: NodeId) -> NodeId {
    let bar = with_reactive_scope(document, || view! { <scrollbar scroll={scroll} /> });
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

    let kind = with_reactive_scope(
        document,
        || view! { <code content={entry.kind.to_owned()} /> },
    );
    let kind = document.shadow_root(kind);

    let detail = with_reactive_scope(
        document,
        || view! { <code content={entry.detail.clone()} /> },
    );
    let detail = document.shadow_root(detail);
    document.set_text_color(detail, TEXT_MUTED);

    let size = with_reactive_scope(document, || view! { <code content={entry.size.clone()} /> });
    let size = document.shadow_root(size);
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

    let node = entry.key.node();
    let hover = state.clone();
    let selection = state.clone();
    let row = with_reactive_scope(document, || {
        view! {
            <click_catcher
                cursor={CursorIcon::PointingHand}
                on_click={Box::new(move |_document: &mut Document| {
                    selection.select(node)
                })}
                on_hover_change={Box::new(move |_document: &mut Document, hovered: bool| {
                    hover.hover(node, hovered);
                })}
            >
                {outline}
            </click_catcher>
        }
    });

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
    let glyph = with_reactive_scope(
        document,
        || view! { <code content={glyph(entry).to_owned()} /> },
    );
    let glyph = document.shadow_root(glyph);
    document.set_text_color(glyph, TEXT_MUTED);
    document.set_text_align(glyph, TextAlign::Center, TextAlign::Center);
    if !entry.expandable {
        return glyph;
    }

    let marker = with_reactive_scope(document, || unstyled::PressableBuilder::default().build());
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
