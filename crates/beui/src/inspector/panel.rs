use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use crate::color::Color32;
use crate::input::CursorIcon;

use crate::base::{ScrollPosition, TextAlign};
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    clone, component, create_memo, create_signal, on_cleanup, view, CenteredRow, ClickCatcher,
    Column, Fill, ForEach, ItemSize, Memo, NodeRef, Outline, Padding, ReadSignal, Row, Scroll,
    Show, Spacer, WriteSignal,
};
use crate::styled::theme::{
    ACCENT, BORDER_WIDTH, CHIP_RADIUS, ON_ACCENT, RADIUS, SCROLLBAR_WIDTH, SEPARATOR_HEIGHT,
    SURFACE, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::styled::{
    Bordered, Button, ButtonVariant, Caption, Checkbox, Code, Heading, ListRow, Scrollbar,
    Separator, Tabs,
};
use crate::unstyled;

use super::tree::{Entry, Key};
use super::{InspectorTab, State};
use crate::{PerformanceSnapshot, PerformanceTimings};

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
const PERFORMANCE_SPACING: f32 = 10.0;
const TIMING_SPACING: f32 = 4.0;

#[derive(Clone, Default, PartialEq)]
pub(crate) struct Summary {
    pub(crate) total: usize,
    pub(crate) picking: bool,
    pub(crate) selection: String,
    pub(crate) bounds: String,
}

#[derive(Clone, Default, PartialEq)]
pub(crate) struct PerformanceSummary {
    samples: String,
    current: PerformanceTimings,
    average: PerformanceTimings,
    peak: PerformanceTimings,
    latest_work: String,
    scene: String,
    cache: String,
}

impl From<PerformanceSnapshot> for PerformanceSummary {
    fn from(snapshot: PerformanceSnapshot) -> Self {
        let samples = match snapshot.samples {
            1 => "1 sample".to_owned(),
            count => format!("{count} samples"),
        };
        let latest = snapshot.latest;
        let (latest_work, scene, cache) = if snapshot.samples == 0 {
            (
                "Waiting for a document update".to_owned(),
                String::new(),
                "Cache: waiting for measurements".to_owned(),
            )
        } else {
            (
                format!(
                    "Latest: {} layout passes | paint {}",
                    latest.layout_passes,
                    if latest.painted { "ran" } else { "cached" }
                ),
                format!("Scene: {} nodes | {} shapes", latest.nodes, latest.shapes),
                format!(
                    "Cache: layout {}% | paint {}%",
                    percentage(snapshot.layout_cache_hits, snapshot.samples),
                    percentage(snapshot.paint_cache_hits, snapshot.samples)
                ),
            )
        };
        Self {
            samples,
            current: latest.timings,
            average: snapshot.average,
            peak: snapshot.peak,
            latest_work,
            scene,
            cache,
        }
    }
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
    pub(crate) set_performance: WriteSignal<PerformanceSummary>,
    pub(crate) set_reveal: WriteSignal<Option<usize>>,
    #[cfg(test)]
    pub(crate) touch_toggle: NodeRef,
    #[cfg(test)]
    pub(crate) tabs: NodeRef,
    #[cfg(test)]
    pub(crate) performance_panel: NodeRef,
    #[cfg(test)]
    pub(crate) rows: Rows,
}

pub(crate) fn build(state: &Rc<State>) -> Panel {
    let (keys, set_keys) = create_signal(Vec::<Key>::new());
    let (entries, set_entries) = create_signal(HashMap::<Key, Entry>::new());
    let (summary, set_summary) = create_signal(Summary::default());
    let (performance, set_performance) = create_signal(PerformanceSummary::default());
    let (tab, set_tab) = create_signal(InspectorTab::default());
    let (position, set_position) = create_signal(ScrollPosition::ZERO);
    let (reveal, set_reveal) = create_signal(None);
    let rows: Rows = Rc::default();
    let touch_toggle = NodeRef::new();
    let touch_toggle_ref = touch_toggle.clone();
    let tabs = NodeRef::new();
    let tabs_ref = tabs.clone();
    let performance_panel = NodeRef::new();
    let performance_panel_ref = performance_panel.clone();
    let touch_emulation = state.touch_emulation.get();
    let touch_state = state.clone();
    let tab_state = state.clone();
    let reset_state = state.clone();

    let mut document = crate::reactive::build(|| {
        let count_text = create_memo({
            let (summary, performance, tab) = (summary.clone(), performance.clone(), tab.clone());
            move || match tab.get() {
                InspectorTab::Performance => {
                    performance.with(|performance| performance.samples.clone())
                }
                _ => total_label(summary.with(|summary| summary.total)),
            }
        });
        let tree_visible = create_memo({
            let tab = tab.clone();
            move || tab.get() != InspectorTab::Performance
        });
        let performance_visible = create_memo(move || tab.get() == InspectorTab::Performance);
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
        let pick_state = state.clone();
        let header_tree_visible = tree_visible.clone();
        let body_tree_visible = tree_visible.clone();
        let footer_tree_visible = tree_visible;
        let body_performance_visible = performance_visible.clone();
        let footer_performance_visible = performance_visible;
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
                                <Show condition={header_tree_visible}>
                                    <PickToggle state={pick_state} picking />
                                </Show>
                            </CenteredRow>
                            <Tabs
                                @node_ref=&tabs_ref
                                labels={vec!["BEUI".to_owned(), "A11y".to_owned(), "Perf".to_owned()]}
                                selected=0
                                on_change={move |index| {
                                    tab_state.set_tab(index);
                                    set_tab.set(InspectorTab::from_index(index));
                                }}
                            />
                        </Column>
                    </Padding>
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <Padding @sizing=ItemSize::Percent(100.0) horizontal=BODY_PADDING vertical=BODY_PADDING>
                        <Column spacing=0.0>
                            <Show @sizing=ItemSize::Percent(100.0) condition={body_tree_visible}>
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
                            </Show>
                            <Show @sizing=ItemSize::Percent(100.0) condition={body_performance_visible}>
                                <PerformancePanel
                                    @node_ref=&performance_panel_ref
                                    performance={performance.clone()}
                                />
                            </Show>
                        </Column>
                    </Padding>
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <Padding horizontal=FOOTER_PADDING vertical=FOOTER_PADDING>
                        <Column spacing=0.0>
                            <Show condition={footer_tree_visible}>
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
                            </Show>
                            <Show condition={footer_performance_visible}>
                                <Button
                                    label="Reset samples"
                                    variant=ButtonVariant::Secondary
                                    on_click={move || reset_state.reset_performance()}
                                />
                            </Show>
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
        set_performance,
        set_reveal,
        #[cfg(test)]
        touch_toggle,
        #[cfg(test)]
        tabs,
        #[cfg(test)]
        performance_panel,
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

#[component]
fn PerformancePanel(performance: ReadSignal<PerformanceSummary>) -> NodeId {
    let (position, set_position) = create_signal(ScrollPosition::ZERO);
    let latest_work = performance_text(&performance, |summary| &summary.latest_work);
    let scene = performance_text(&performance, |summary| &summary.scene);
    let cache = performance_text(&performance, |summary| &summary.cache);
    let total = timing_values(&performance, |timings| timings.total);
    let layout = timing_values(&performance, |timings| timings.layout);
    let interaction = timing_values(&performance, |timings| timings.interaction);
    let paint = timing_values(&performance, |timings| timings.paint);
    let accessibility = timing_values(&performance, |timings| timings.accessibility);
    let other = timing_values(&performance, |timings| timings.other);
    view! {
        <Row spacing=BODY_SPACING>
            <Scroll
                @sizing=ItemSize::Percent(100.0)
                focus_color=ACCENT
                on_change={move |value| set_position.set(value)}
            >
                <Column spacing=PERFORMANCE_SPACING>
                    <Column spacing=FOOTER_SPACING>
                        <Code content={latest_work} />
                        <Code content={scene} color=TEXT_MUTED />
                        <Code content={cache} color=TEXT_MUTED />
                    </Column>
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <Column spacing=TIMING_SPACING>
                        <CenteredRow spacing=TIMING_SPACING>
                            <Heading @sizing=ItemSize::Percent(100.0) content="CPU time" />
                            <Caption content="milliseconds" />
                        </CenteredRow>
                        <TimingHeader />
                        <TimingRow label="Document" values={total} />
                        <TimingRow label="Layout" values={layout} />
                        <TimingRow label="Interaction" values={interaction} />
                        <TimingRow label="Paint" values={paint} />
                        <TimingRow label="Accessibility" values={accessibility} />
                        <TimingRow label="Other" values={other} />
                    </Column>
                </Column>
            </Scroll>
            <Scrollbar @sizing=ItemSize::Fixed(SCROLLBAR_WIDTH) position />
        </Row>
    }
}

#[component]
fn TimingHeader() -> NodeId {
    view! {
        <Row spacing=TIMING_SPACING>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <Caption @sizing=ItemSize::Fixed(54.0) content="Current" align=TextAlign::End />
            <Caption @sizing=ItemSize::Fixed(54.0) content="Average" align=TextAlign::End />
            <Caption @sizing=ItemSize::Fixed(54.0) content="Peak" align=TextAlign::End />
        </Row>
    }
}

#[component]
fn TimingRow(label: String, values: [Memo<String>; 3]) -> NodeId {
    let [current, average, peak] = values;
    view! {
        <Row spacing=TIMING_SPACING>
            <Caption @sizing=ItemSize::Percent(100.0) content={label} />
            <Code @sizing=ItemSize::Fixed(54.0) content={current} align=TextAlign::End />
            <Code @sizing=ItemSize::Fixed(54.0) content={average} align=TextAlign::End />
            <Code @sizing=ItemSize::Fixed(54.0) content={peak} align=TextAlign::End />
        </Row>
    }
}

fn performance_text(
    performance: &ReadSignal<PerformanceSummary>,
    read: impl Fn(&PerformanceSummary) -> &String + 'static,
) -> Memo<String> {
    let performance = performance.clone();
    create_memo(move || performance.with(|summary| read(summary).clone()))
}

fn timing_values(
    performance: &ReadSignal<PerformanceSummary>,
    read: impl Fn(PerformanceTimings) -> Duration + Clone + 'static,
) -> [Memo<String>; 3] {
    let timing = |select: fn(&PerformanceSummary) -> PerformanceTimings| {
        let performance = performance.clone();
        let read = read.clone();
        create_memo(move || format_duration(read(performance.with(select))))
    };
    [
        timing(|summary| summary.current),
        timing(|summary| summary.average),
        timing(|summary| summary.peak),
    ]
}

fn format_duration(duration: Duration) -> String {
    format!("{:.3}", duration.as_secs_f64() * 1_000.0)
}

fn percentage(count: usize, total: usize) -> usize {
    count.saturating_mul(100) / total.max(1)
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
