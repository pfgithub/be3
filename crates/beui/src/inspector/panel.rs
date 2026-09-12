use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::color::Color32;
use crate::input::CursorIcon;

use crate::base::{ScrollPosition, TextAlign};
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    clone, component, create_memo, create_signal, on_cleanup, view, CenteredRow, ClickCatcher,
    Column, Fill, ForEach, ItemSize, Memo, NodeRef, Outline, Padding, ReadSignal, Row, Scroll,
    Spacer, WriteSignal,
};
use crate::styled::theme::{
    ACCENT, BORDER_WIDTH, CHIP_RADIUS, ON_ACCENT, RADIUS, SCROLLBAR_WIDTH, SEPARATOR_HEIGHT,
    SURFACE, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::styled::{
    Bordered, Caption, Checkbox, Code, Heading, ListRow, Scrollbar, Separator, Tabs,
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
    pub(crate) tabs: NodeRef,
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
    let tabs = NodeRef::new();
    let tabs_ref = tabs.clone();
    let touch_emulation = state.touch_emulation.get();
    let touch_state = state.clone();
    let tab_state = state.clone();

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
        <Row spacing=0.0>
            <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
            <Fill @sizing=ItemSize::Percent(100.0) color=SURFACE radius=0>
                <Column spacing=0.0>
                    <Padding horizontal=HEADER_PADDING vertical=HEADER_PADDING>
                        <Column spacing=HEADER_SPACING>
                            <CenteredRow spacing=HEADER_SPACING>
                                <Heading content="Inspector" />
                                <Caption @sizing=ItemSize::Percent(100.0) content={count_text} align=TextAlign::End />
                                <PickToggle state={state.clone()} picking />
                            </CenteredRow>
                            <Tabs
                                @node_ref=&tabs_ref
                                labels={vec!["BEUI".to_owned(), "AccessKit".to_owned()]}
                                selected=0
                                on_change={move |index| tab_state.set_tree(index)}
                            />
                        </Column>
                    </Padding>
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <Padding @sizing=ItemSize::Percent(100.0) horizontal=BODY_PADDING vertical=BODY_PADDING>
                        <Row spacing=BODY_SPACING>
                            <Scroll @sizing=ItemSize::Percent(100.0)
                                focus_color=ACCENT
                                reveal
                                on_change={move |value| set_position.set(value)}
                            >
                                <ForEach spacing=0.0 items={keys} key={|key: Key| key}>
                                    {move |key: Key| view! {
                                        <TreeRow
                                            row_key={key}
                                            entries={entries.clone()}
                                            state={list_state.clone()}
                                            rows={list_rows.clone()}
                                        />
                                    }}
                                </ForEach>
                            </Scroll>
                            <Scrollbar @sizing=ItemSize::Fixed(SCROLLBAR_WIDTH) position />
                        </Row>
                    </Padding>
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <Padding horizontal=FOOTER_PADDING vertical=FOOTER_PADDING>
                        <Column spacing=FOOTER_SPACING>
                            <Checkbox
                                @node_ref=&touch_toggle_ref
                                label="Emulate touch with mouse"
                                checked={touch_emulation}
                                on_change={move |enabled| touch_state.touch_emulation.set(enabled)}
                            />
                            <Code content={selection_text} />
                            <Code content={bounds_text} color=TEXT_MUTED />
                        </Column>
                    </Padding>
                </Column>
            </Fill>
        </Row>
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
        tabs,
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
fn PickToggle(state: Rc<State>, picking: Memo<bool>) -> NodeId {
    let label_color = create_memo(clone!(picking -> move || toggle_text(picking.get())));
    let fill_color = create_memo(move || toggle_fill(picking.get()));
    let picker = state;
    view! {
        <unstyled::Pressable on_click={move || picker.toggle_picking()}>
            <Bordered corner_radius=CHIP_RADIUS>
                <Fill color={fill_color} radius=CHIP_RADIUS>
                    <Padding
                        horizontal=TOGGLE_PADDING_HORIZONTAL
                        vertical=TOGGLE_PADDING_VERTICAL
                    >
                        <Code
                            content="Pick"
                            align=TextAlign::Center
                            color={label_color}
                        />
                    </Padding>
                </Fill>
            </Bordered>
        </unstyled::Pressable>
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
fn TreeRow(row_key: Key, entries: Entries, state: Rc<State>, rows: Rows) -> NodeId {
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
        <ClickCatcher
            @node_ref=&row
            cursor=CursorIcon::PointingHand
            on_click={move || selection.select(node)}
            on_hover_change={move |hovered| hover.hover(node, hovered)}
        >
            <Outline
                color=ACCENT
                width=BORDER_WIDTH
                radius=RADIUS
                offset=0.0
                visible={selected}
            >
                <ListRow>
                    <CenteredRow spacing=ROW_SPACING>
                        <Spacer @sizing={indent} />
                        <unstyled::Pressable @sizing=ItemSize::Fixed(MARKER_WIDTH)
                            @node_ref=&marker
                            enabled={expandable}
                            on_click={move || {
                                expansion.set_expanded(key, !expanded.get_untracked());
                            }}
                        >
                            <Code content={glyph} color=TEXT_MUTED align=TextAlign::Center />
                        </unstyled::Pressable>
                        <Code content={kind} />
                        <Code @sizing=ItemSize::Percent(100.0) content={detail} color=TEXT_MUTED />
                        <Code content={size} color=TEXT_MUTED align=TextAlign::End />
                    </CenteredRow>
                </ListRow>
            </Outline>
        </ClickCatcher>
    }
}

fn glyph(expandable: bool, expanded: bool) -> &'static str {
    match (expandable, expanded) {
        (true, true) => "-",
        (true, false) => "+",
        (false, _) => "",
    }
}
