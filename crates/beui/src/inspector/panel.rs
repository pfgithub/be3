use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::color::Color32;
use crate::input::CursorIcon;

use crate::base::{ScrollPosition, TextAlign};
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    clone, component, create_memo, create_signal, on_cleanup, view, CenteredRowBuilder,
    ClickCatcherBuilder, ColumnBuilder, FillBuilder, ForEachBuilder, ItemSize, Memo, NodeRef,
    OutlineBuilder, PaddingBuilder, ReadSignal, RowBuilder, ScrollBuilder, SpacerBuilder,
    WriteSignal,
};
use crate::styled::theme::{
    ACCENT, BORDER_WIDTH, CHIP_RADIUS, ON_ACCENT, RADIUS, SCROLLBAR_WIDTH, SEPARATOR_HEIGHT,
    SURFACE, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::styled::{
    BorderedBuilder, CaptionBuilder, CheckboxBuilder, CodeBuilder, HeadingBuilder, ListRowBuilder,
    ScrollbarBuilder, SeparatorBuilder,
};
use crate::unstyled;

use super::tree::{Entry, Key};
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

#[derive(Clone, Default, PartialEq)]
pub(crate) struct Summary {
    pub(crate) total: usize,
    pub(crate) picking: bool,
    pub(crate) selection: String,
    pub(crate) bounds: String,
}

pub(crate) struct Row {
    #[cfg(test)]
    pub(crate) row: NodeRef,
    #[cfg(test)]
    pub(crate) marker: NodeRef,
}

type Rows = Rc<RefCell<HashMap<Key, Row>>>;

type Entries = ReadSignal<HashMap<Key, Entry>>;

pub(crate) struct Panel {
    pub(crate) document: Document,
    pub(crate) set_keys: WriteSignal<Vec<Key>>,
    pub(crate) set_entries: WriteSignal<HashMap<Key, Entry>>,
    pub(crate) set_summary: WriteSignal<Summary>,
    pub(crate) set_reveal: WriteSignal<Option<usize>>,
    #[cfg(test)]
    pub(crate) touch_toggle: NodeRef,
    #[cfg(test)]
    pub(crate) rows: Rows,
}

pub(crate) fn build(state: &Rc<State>) -> Panel {
    let (keys, set_keys) = create_signal(Vec::<Key>::new());
    let (entries, set_entries) = create_signal(HashMap::<Key, Entry>::new());
    let (summary, set_summary) = create_signal(Summary::default());
    let (position, set_position) = create_signal(ScrollPosition::ZERO);
    let (reveal, set_reveal) = create_signal(None);
    let rows: Rows = Rc::default();
    let touch_toggle = NodeRef::new();
    let touch_toggle_ref = touch_toggle.clone();
    let touch_emulation = state.touch_emulation.get();
    let touch_state = state.clone();

    let mut document = crate::reactive::build(|| {
        let count_text = create_memo({
            let summary = summary.clone();
            move || total_label(summary.with(|summary| summary.total))
        });
        let picking = create_memo({
            let summary = summary.clone();
            move || summary.with(|summary| summary.picking)
        });
        let selection_text = create_memo({
            let summary = summary.clone();
            move || summary.with(|summary| summary.selection.clone())
        });
        let bounds_text = create_memo(move || summary.with(|summary| summary.bounds.clone()));
        let (list_state, list_rows) = (state.clone(), rows.clone());
        view! {
        <row spacing=0.0>
            <separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
            <fill @sizing=ItemSize::Percent(100.0) color=SURFACE radius=0>
                <column spacing=0.0>
                    <padding horizontal=HEADER_PADDING vertical=HEADER_PADDING>
                        <centered_row spacing=HEADER_SPACING>
                            <heading content="Inspector" />
                            <caption @sizing=ItemSize::Percent(100.0) content={count_text} align=TextAlign::End />
                            <pick_toggle state={state.clone()} picking />
                        </centered_row>
                    </padding>
                    <separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <padding @sizing=ItemSize::Percent(100.0) horizontal=BODY_PADDING vertical=BODY_PADDING>
                        <row spacing=BODY_SPACING>
                            <scroll @sizing=ItemSize::Percent(100.0)
                                focus_color=ACCENT
                                reveal
                                on_change={move |value| set_position.set(value)}
                            >
                                <for_each spacing=0.0 items={keys} key={|key: Key| key}>
                                    {move |key: Key| view! {
                                        <tree_row
                                            row_key={key}
                                            entries={entries.clone()}
                                            state={list_state.clone()}
                                            rows={list_rows.clone()}
                                        />
                                    }}
                                </for_each>
                            </scroll>
                            <scrollbar @sizing=ItemSize::Fixed(SCROLLBAR_WIDTH) position />
                        </row>
                    </padding>
                    <separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <padding horizontal=FOOTER_PADDING vertical=FOOTER_PADDING>
                        <column spacing=FOOTER_SPACING>
                            <checkbox
                                @node_ref=&touch_toggle_ref
                                label="Emulate touch with mouse"
                                checked={touch_emulation}
                                on_change={move |enabled| touch_state.touch_emulation.set(enabled)}
                            />
                            <code content={selection_text} />
                            <code content={bounds_text} color=TEXT_MUTED />
                        </column>
                    </padding>
                </column>
            </fill>
        </row>
        }
    });
    document.inspectable = false;

    Panel {
        document,
        set_keys,
        set_entries,
        set_summary,
        set_reveal,
        #[cfg(test)]
        touch_toggle,
        #[cfg(test)]
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

#[component]
fn pick_toggle(state: Rc<State>, picking: Memo<bool>) -> NodeId {
    let label_color = create_memo(clone!(picking -> move || toggle_text(picking.get())));
    let fill_color = create_memo(move || toggle_fill(picking.get()));
    let picker = state;
    view! {
        <unstyled::pressable on_click={move || picker.toggle_picking()}>
            <bordered corner_radius=CHIP_RADIUS>
                <fill color={fill_color} radius=CHIP_RADIUS>
                    <padding
                        horizontal=TOGGLE_PADDING_HORIZONTAL
                        vertical=TOGGLE_PADDING_VERTICAL
                    >
                        <code
                            content="Pick"
                            align=TextAlign::Center
                            color={label_color}
                        />
                    </padding>
                </fill>
            </bordered>
        </unstyled::pressable>
    }
}

fn entry_field<T: Clone + Default + PartialEq + 'static>(
    entries: &Entries,
    key: Key,
    read: impl Fn(&Entry) -> T + 'static,
) -> Memo<T> {
    let entries = entries.clone();
    create_memo(move || entries.with(|entries| entries.get(&key).map(&read).unwrap_or_default()))
}

#[component]
fn tree_row(row_key: Key, entries: Entries, state: Rc<State>, rows: Rows) -> NodeId {
    let key = row_key;
    let node = key.node();
    let indent = entry_field(&entries, key, |entry| {
        ItemSize::Fixed(entry.depth as f32 * INDENT)
    });
    let kind = entry_field(&entries, key, |entry| entry.kind.to_owned());
    let detail = entry_field(&entries, key, |entry| entry.detail.clone());
    let size = entry_field(&entries, key, |entry| entry.size.clone());
    let selected = entry_field(&entries, key, |entry| entry.selected);
    let expandable = entry_field(&entries, key, |entry| entry.expandable);
    let expanded = entry_field(&entries, key, |entry| entry.expanded);
    let glyph = create_memo({
        let (expandable, expanded) = (expandable.clone(), expanded.clone());
        move || glyph(expandable.get(), expanded.get()).to_owned()
    });

    let (row, marker) = (NodeRef::new(), NodeRef::new());
    rows.borrow_mut().insert(
        key,
        Row {
            #[cfg(test)]
            row: row.clone(),
            #[cfg(test)]
            marker: marker.clone(),
        },
    );
    on_cleanup(move || {
        rows.borrow_mut().remove(&key);
    });

    let (hover, selection, expansion) = (state.clone(), state.clone(), state);
    view! {
        <click_catcher
            @node_ref=&row
            cursor=CursorIcon::PointingHand
            on_click={move || selection.select(node)}
            on_hover_change={move |hovered| hover.hover(node, hovered)}
        >
            <outline
                color=ACCENT
                width=BORDER_WIDTH
                radius=RADIUS
                offset=0.0
                visible={selected}
            >
                <list_row>
                    <centered_row spacing=ROW_SPACING>
                        <spacer @sizing={indent} />
                        <unstyled::pressable @sizing=ItemSize::Fixed(MARKER_WIDTH)
                            @node_ref=&marker
                            enabled={expandable}
                            on_click={move || {
                                expansion.set_expanded(key, !expanded.get_untracked());
                            }}
                        >
                            <code content={glyph} color=TEXT_MUTED align=TextAlign::Center />
                        </unstyled::pressable>
                        <code content={kind} />
                        <code @sizing=ItemSize::Percent(100.0) content={detail} color=TEXT_MUTED />
                        <code content={size} color=TEXT_MUTED align=TextAlign::End />
                    </centered_row>
                </list_row>
            </outline>
        </click_catcher>
    }
}

fn glyph(expandable: bool, expanded: bool) -> &'static str {
    match (expandable, expanded) {
        (true, true) => "-",
        (true, false) => "+",
        (false, _) => "",
    }
}
