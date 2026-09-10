use std::rc::Rc;

use crate::color::Color32;
use crate::input::CursorIcon;

use crate::base::TextAlign;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    create_signal, intrinsic, view, with_reactive_scope, CenteredRowBuilder, ClickCatcherBuilder,
    ColumnBuilder, FillBuilder, OutlineBuilder, PaddingBuilder, Prop, ReadSignal, RowBuilder,
    ScrollBuilder, SpacerBuilder, WriteSignal,
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
    let (count_text, set_count) = create_signal(total_label(summary.total));
    let (picking, set_picking) = create_signal(summary.picking);
    let (selection_text, set_selection) = create_signal(summary.selection.clone());
    let (bounds_text, set_bounds) = create_signal(summary.bounds.clone());

    let rows: Vec<Row> = entries.iter().map(|entry| row(entry, state)).collect();
    let items: Vec<NodeId> = rows.iter().map(|row| row.row).collect();
    let scroll = view! {
        <scroll
            offset={offset}
            focus_color={ACCENT}
            children={items.into_iter().map(intrinsic).collect::<Vec<_>>()}
        />
    };

    let panel = view! {
        <row spacing={0.0}>
            @fixed(SEPARATOR_HEIGHT) <separator />
            @percent(100.0) <fill color={SURFACE} radius={0}>
                <column spacing={0.0}>
                    <padding horizontal={HEADER_PADDING} vertical={HEADER_PADDING}>
                        <centered_row spacing={HEADER_SPACING}>
                            <heading content={"Inspector".to_string()} />
                            @percent(100.0) <caption content={count_text} align={TextAlign::End} />
                            {pick_toggle(state, &picking)}
                        </centered_row>
                    </padding>
                    @fixed(SEPARATOR_HEIGHT) <separator />
                    @percent(100.0) <padding horizontal={BODY_PADDING} vertical={BODY_PADDING}>
                        <row spacing={BODY_SPACING}>
                            @percent(100.0) {scroll}
                            @fixed(SCROLLBAR_WIDTH) <scrollbar scroll={scroll} />
                        </row>
                    </padding>
                    @fixed(SEPARATOR_HEIGHT) <separator />
                    <padding horizontal={FOOTER_PADDING} vertical={FOOTER_PADDING}>
                        <column spacing={FOOTER_SPACING}>
                            <code content={selection_text} />
                            <code content={bounds_text} color={TEXT_MUTED} />
                        </column>
                    </padding>
                </column>
            </fill>
        </row>
    };

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

fn row(entry: &Entry, state: &Rc<State>) -> Row {
    let marker = marker(entry, state);
    let (detail_text, set_detail) = create_signal(entry.detail.clone());
    let (size_text, set_size) = create_signal(entry.size.clone());
    let indent = entry.depth as f32 * INDENT;

    let outline_cell = std::cell::Cell::new(None);
    let node = entry.key.node();
    let hover = state.clone();
    let selection = state.clone();
    let row = view! {
        <click_catcher
            cursor={CursorIcon::PointingHand}
            on_click={move || selection.select(node)}
            on_hover_change={move |hovered| hover.hover(node, hovered)}
        >
            {{
                let outline = view! {
                    <outline
                        color={ACCENT}
                        width={BORDER_WIDTH}
                        radius={RADIUS}
                        offset={0.0}
                        visible={entry.selected}
                    >
                        <list_row>
                            <centered_row spacing={ROW_SPACING}>
                                @fixed(indent) <spacer />
                                @fixed(MARKER_WIDTH) {marker}
                                <code content={entry.kind.to_owned()} />
                                @percent(100.0) <code content={detail_text} color={TEXT_MUTED} />
                                <code content={size_text} color={TEXT_MUTED} align={TextAlign::End} />
                            </centered_row>
                        </list_row>
                    </outline>
                };
                outline_cell.set(Some(outline));
                outline
            }}
        </click_catcher>
    };
    let outline = outline_cell.get().expect("row outline not yet built");

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
