use std::rc::Rc;

use crate::color::Color32;
use crate::input::CursorIcon;

use crate::base::{ItemSize, TextAlign};
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    create_signal, view, with_document, with_reactive_scope, ClickCatcherBuilder, WriteSignal,
};
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

struct Built {
    scroll: NodeId,
    set_count: WriteSignal<String>,
    toggle: NodeId,
    toggle_label: NodeId,
    selection: NodeId,
    bounds: NodeId,
    rows: Vec<Row>,
    panel: NodeId,
}

pub(crate) fn build(entries: &[Entry], summary: &Summary, state: &Rc<State>, offset: f32) -> Panel {
    let mut document = Document::new();
    document.inspectable = false;

    let built = with_reactive_scope(&mut document, || {
        build_tree(entries, summary, state, offset)
    });
    document.set_root(built.panel);

    Panel {
        document,
        scroll: built.scroll,
        set_count: built.set_count,
        toggle: built.toggle,
        toggle_label: built.toggle_label,
        selection: built.selection,
        bounds: built.bounds,
        rows: built.rows,
    }
}

fn build_tree(entries: &[Entry], summary: &Summary, state: &Rc<State>, offset: f32) -> Built {
    let (count, set_count) = count_label(summary.total);
    let (toggle, toggle_fill, toggle_label) = pick_toggle(state, summary.picking);
    let header = header(count, toggle);

    let scroll = with_document(Document::create_scroll);
    let rows: Vec<Row> = entries
        .iter()
        .map(|entry| {
            let row = row(entry, state);
            with_document(|document| document.append_scroll_item(scroll, row.row));
            row
        })
        .collect();
    with_document(|document| document.set_scroll_offset(scroll, offset));
    let body = body(scroll);

    let selection = view! { <code content={summary.selection.clone()} /> };
    let selection = with_document(|document| document.shadow_root(selection));
    let bounds = view! { <code content={summary.bounds.clone()} /> };
    let bounds = with_document(|document| document.shadow_root(bounds));
    with_document(|document| document.set_text_color(bounds, TEXT_MUTED));
    let footer = footer(selection, bounds);

    let column = unstyled::column(0.0);
    let above = view! { <separator /> };
    let below = view! { <separator /> };
    with_document(|document| {
        document.append_child(column, header, ItemSize::Intrinsic);
        document.append_child(column, above, ItemSize::Fixed(SEPARATOR_HEIGHT));
        document.append_child(column, body, ItemSize::Percent(100.0));
        document.append_child(column, below, ItemSize::Fixed(SEPARATOR_HEIGHT));
        document.append_child(column, footer, ItemSize::Intrinsic);
    });

    let surface = with_document(|document| {
        let surface = document.create_fill(SURFACE, 0);
        document.set_fill_child(surface, column);
        surface
    });

    let edge = view! { <separator /> };
    let panel = unstyled::row(0.0);
    with_document(|document| {
        document.append_child(panel, edge, ItemSize::Fixed(SEPARATOR_HEIGHT));
        document.append_child(panel, surface, ItemSize::Percent(100.0));
    });

    Built {
        scroll,
        set_count,
        toggle: toggle_fill,
        toggle_label,
        selection,
        bounds,
        rows,
        panel,
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

fn count_label(total: usize) -> (NodeId, WriteSignal<String>) {
    let (count_text, set_count_text) = create_signal(total_label(total));
    let count = view! { <caption content={count_text} /> };
    with_document(|document| {
        let count_text_node = document.shadow_root(count);
        document.set_text_align(count_text_node, TextAlign::End, TextAlign::Center);
    });
    (count, set_count_text)
}

fn header(count: NodeId, toggle: NodeId) -> NodeId {
    let title = view! { <heading content={"Inspector".to_string()} /> };
    let line = unstyled::centered_row(HEADER_SPACING);
    with_document(|document| {
        document.append_child(line, title, ItemSize::Intrinsic);
        document.append_child(line, count, ItemSize::Percent(100.0));
        document.append_child(line, toggle, ItemSize::Intrinsic);
        let padding = document.create_padding(HEADER_PADDING, HEADER_PADDING);
        document.set_padding_child(padding, line);
        padding
    })
}

fn pick_toggle(state: &Rc<State>, picking: bool) -> (NodeId, NodeId, NodeId) {
    let label = view! { <code content={"Pick".to_owned()} /> };
    let label = with_document(|document| {
        let label = document.shadow_root(label);
        document.set_text_align(label, TextAlign::Center, TextAlign::Center);
        document.set_text_color(label, toggle_text(picking));
        label
    });

    let fill = with_document(|document| {
        let padding = document.create_padding(TOGGLE_PADDING_HORIZONTAL, TOGGLE_PADDING_VERTICAL);
        document.set_padding_child(padding, label);

        let fill = document.create_fill(toggle_fill(picking), CHIP_RADIUS);
        document.set_fill_child(fill, padding);
        fill
    });

    let bordered = view! { <bordered corner_radius={CHIP_RADIUS}>{fill}</bordered> };
    let pressable = view! { <unstyled::pressable>{bordered}</unstyled::pressable> };

    let picker = state.clone();
    unstyled::set_pressable_on_click(pressable, move |_document| picker.toggle_picking());

    (pressable, fill, label)
}

fn body(scroll: NodeId) -> NodeId {
    let bar = view! { <scrollbar scroll={scroll} /> };
    let area = unstyled::row(BODY_SPACING);
    with_document(|document| {
        document.append_child(area, scroll, ItemSize::Percent(100.0));
        document.append_child(area, bar, ItemSize::Fixed(SCROLLBAR_WIDTH));
        let padding = document.create_padding(BODY_PADDING, BODY_PADDING);
        document.set_padding_child(padding, area);
        padding
    })
}

fn footer(selection: NodeId, bounds: NodeId) -> NodeId {
    let column = unstyled::column(FOOTER_SPACING);
    with_document(|document| {
        document.append_child(column, selection, ItemSize::Intrinsic);
        document.append_child(column, bounds, ItemSize::Intrinsic);
        let padding = document.create_padding(FOOTER_PADDING, FOOTER_PADDING);
        document.set_padding_child(padding, column);
        padding
    })
}

fn row(entry: &Entry, state: &Rc<State>) -> Row {
    let indent = with_document(|document| document.create_fill(Color32::TRANSPARENT, 0));
    let marker = marker(entry, state);

    let kind = view! { <code content={entry.kind.to_owned()} /> };
    let kind = with_document(|document| document.shadow_root(kind));

    let detail = view! { <code content={entry.detail.clone()} /> };
    let detail = with_document(|document| {
        let detail = document.shadow_root(detail);
        document.set_text_color(detail, TEXT_MUTED);
        detail
    });

    let size = view! { <code content={entry.size.clone()} /> };
    let size = with_document(|document| {
        let size = document.shadow_root(size);
        document.set_text_color(size, TEXT_MUTED);
        document.set_text_align(size, TextAlign::End, TextAlign::Center);
        size
    });

    let line = unstyled::centered_row(ROW_SPACING);
    with_document(|document| {
        document.append_child(line, indent, ItemSize::Fixed(entry.depth as f32 * INDENT));
        document.append_child(line, marker, ItemSize::Fixed(MARKER_WIDTH));
        document.append_child(line, kind, ItemSize::Intrinsic);
        document.append_child(line, detail, ItemSize::Percent(100.0));
        document.append_child(line, size, ItemSize::Intrinsic);
    });

    let list_row = view! { <list_row>{line}</list_row> };
    let outline = with_document(|document| {
        let outline = document.create_outline(ACCENT, BORDER_WIDTH, RADIUS, 0.0);
        document.set_outline_visible(outline, entry.selected);
        document.set_outline_child(outline, list_row);
        outline
    });

    let node = entry.key.node();
    let hover = state.clone();
    let selection = state.clone();
    let row = view! {
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
    };

    Row {
        row,
        #[cfg(test)]
        marker,
        outline,
        detail,
        size,
    }
}

fn marker(entry: &Entry, state: &Rc<State>) -> NodeId {
    let glyph_node = view! { <code content={glyph(entry).to_owned()} /> };
    let glyph_node = with_document(|document| {
        let glyph_node = document.shadow_root(glyph_node);
        document.set_text_color(glyph_node, TEXT_MUTED);
        document.set_text_align(glyph_node, TextAlign::Center, TextAlign::Center);
        glyph_node
    });
    if !entry.expandable {
        return glyph_node;
    }

    let marker = view! { <unstyled::pressable>{glyph_node}</unstyled::pressable> };

    let expansion = state.clone();
    let key = entry.key;
    let expanded = entry.expanded;
    unstyled::set_pressable_on_click(marker, move |_document| {
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
