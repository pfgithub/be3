use std::rc::Rc;

use crate::color::Color32;
use crate::input::CursorIcon;

use crate::base::{ItemSize, TextAlign};
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    create_signal, view, with_document, with_reactive_scope, ClickCatcherBuilder, FillBuilder,
    PaddingBuilder, Prop, ReadSignal, WriteSignal,
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
    pub(crate) set_detail: WriteSignal<String>,
    pub(crate) set_size: WriteSignal<String>,
}

pub(crate) struct Panel {
    pub(crate) document: Document,
    pub(crate) scroll: NodeId,
    pub(crate) set_count: WriteSignal<String>,
    pub(crate) set_picking: WriteSignal<bool>,
    pub(crate) set_selection: WriteSignal<String>,
    pub(crate) set_bounds: WriteSignal<String>,
    pub(crate) rows: Vec<Row>,
}

struct Built {
    scroll: NodeId,
    set_count: WriteSignal<String>,
    set_picking: WriteSignal<bool>,
    set_selection: WriteSignal<String>,
    set_bounds: WriteSignal<String>,
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
        set_picking: built.set_picking,
        set_selection: built.set_selection,
        set_bounds: built.set_bounds,
        rows: built.rows,
    }
}

fn build_tree(entries: &[Entry], summary: &Summary, state: &Rc<State>, offset: f32) -> Built {
    let (count, set_count) = count_label(summary.total);
    let (picking, set_picking) = create_signal(summary.picking);
    let toggle = pick_toggle(state, &picking);
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

    let (selection_text, set_selection) = create_signal(summary.selection.clone());
    let (bounds_text, set_bounds) = create_signal(summary.bounds.clone());
    let selection = view! { <code content={selection_text} /> };
    let bounds = view! { <code content={bounds_text} color={TEXT_MUTED} /> };
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
        set_picking,
        set_selection,
        set_bounds,
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
    let count = view! { <caption content={count_text} align={TextAlign::End} /> };
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

fn pick_toggle(state: &Rc<State>, picking: &ReadSignal<bool>) -> NodeId {
    let label_color = {
        let picking = picking.clone();
        Prop::Dynamic(Box::new(move || toggle_text(picking.get())))
    };
    let fill_color = {
        let picking = picking.clone();
        Prop::Dynamic(Box::new(move || toggle_fill(picking.get())))
    };
    let picker = state.clone();
    view! {
        <unstyled::pressable on_click={move || picker.toggle_picking()}>
            <bordered corner_radius={CHIP_RADIUS}>
                <fill color={fill_color} radius={CHIP_RADIUS}>
                    <padding
                        horizontal={TOGGLE_PADDING_HORIZONTAL}
                        vertical={TOGGLE_PADDING_VERTICAL}
                    >
                        <code
                            content={"Pick".to_owned()}
                            align={TextAlign::Center}
                            color={label_color}
                        />
                    </padding>
                </fill>
            </bordered>
        </unstyled::pressable>
    }
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
    let (detail_text, set_detail) = create_signal(entry.detail.clone());
    let (size_text, set_size) = create_signal(entry.size.clone());
    let detail = view! { <code content={detail_text} color={TEXT_MUTED} /> };
    let size = view! {
        <code content={size_text} color={TEXT_MUTED} align={TextAlign::End} />
    };

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
            on_click={move || selection.select(node)}
            on_hover_change={move |hovered| hover.hover(node, hovered)}
        >
            {outline}
        </click_catcher>
    };

    Row {
        row,
        #[cfg(test)]
        marker,
        outline,
        set_detail,
        set_size,
    }
}

fn marker(entry: &Entry, state: &Rc<State>) -> NodeId {
    let glyph_node = view! {
        <code
            content={glyph(entry).to_owned()}
            color={TEXT_MUTED}
            align={TextAlign::Center}
        />
    };
    if !entry.expandable {
        return glyph_node;
    }

    let expansion = state.clone();
    let key = entry.key;
    let expanded = entry.expanded;
    view! {
        <unstyled::pressable on_click={move || expansion.set_expanded(key, !expanded)}>
            {glyph_node}
        </unstyled::pressable>
    }
}

fn glyph(entry: &Entry) -> &'static str {
    match (entry.expandable, entry.expanded) {
        (true, true) => "-",
        (true, false) => "+",
        (false, _) => "",
    }
}
