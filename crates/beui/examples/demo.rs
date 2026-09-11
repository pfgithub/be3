use beui::reactive::{
    create_memo, create_selector, create_signal, view, with_reactive_scope, CenteredRowBuilder,
    ColumnBuilder, FillBuilder, OutlineBuilder, PaddingBuilder, Prop, ReadSignal, RowBuilder,
    Selector, ShowBuilder, SpacerBuilder, VirtualListBuilder, VisibilityBuilder, WriteSignal,
};
use beui::styled::theme::{
    ACCENT, ACCENT_SOFT, BACKGROUND, RADIUS, SCROLLBAR_WIDTH, SEPARATOR_HEIGHT, SURFACE,
    SURFACE_RAISED, TEXT_MUTED,
};
use beui::styled::{
    AccordionBuilder, BodyBuilder, ButtonBuilder, ButtonVariant, CaptionBuilder, CardBuilder,
    CheckboxBuilder, ContextMenuBuilder, DisplayBuilder, HeadingBuilder, ListboxBuilder,
    ParagraphBuilder, ProgressBuilder, RadioGroupBuilder, ScrollbarBuilder, SelectBuilder,
    SeparatorBuilder, ShortcutBuilder, SliderBuilder, SwitchBuilder, TabsBuilder, TextInputBuilder,
    TitleBuilder, ToggleButtonBuilder,
};
use beui::{unstyled, Color32, Context, Document, NodeId, Rect, ScrollPosition, TextAlign};
use beui_macros::component;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("beui demo", DemoApp::new())
}

const HEADER_HEIGHT: f32 = 64.0;
const HEADER_PADDING: f32 = 20.0;
const BODY_PADDING: f32 = 20.0;
const ICON_BUTTON_WIDTH: f32 = 44.0;
const ROW_COUNT: usize = 10_000;
const ROW_HEIGHT: f32 = 34.0;
const COMPACT_ROW_HEIGHT: f32 = 25.0;
const ROW_PADDING_HORIZONTAL: f32 = 12.0;
const ROW_PADDING_VERTICAL: f32 = 9.0;
const COMPACT_ROW_PADDING_VERTICAL: f32 = 4.0;

struct DemoApp {
    document: Document,
}

impl DemoApp {
    fn new() -> Self {
        let mut document = Document::new();
        let (count, set_count) = create_signal(0i64);

        let root = with_reactive_scope(&mut document, || {
            view! {
                <fill color={BACKGROUND} radius={0}>
                    <column spacing={0.0}>
                        @fixed(HEADER_HEIGHT) <demo_header set_count={set_count} />
                        @fixed(SEPARATOR_HEIGHT) <separator/>
                        @percent(100.0) <demo_body count={count} />
                    </column>
                </fill>
            }
        });
        document.set_root(root);

        Self { document }
    }
}

impl beui::App for DemoApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        BACKGROUND
    }
}

#[derive(Clone)]
struct Rows {
    set_status: WriteSignal<String>,
    selection: Selector<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
    timings: ReadSignal<bool>,
    set_timings: WriteSignal<bool>,
    compact: ReadSignal<bool>,
    set_compact: WriteSignal<bool>,
}

impl Rows {
    fn new(set_status: WriteSignal<String>) -> Self {
        let (selected, set_selected) = create_signal(None);
        let selection = create_selector(move || selected.get());
        let (timings, set_timings) = create_signal(true);
        let (compact, set_compact) = create_signal(false);
        Self {
            set_status,
            selection,
            set_selected,
            timings,
            set_timings,
            compact,
            set_compact,
        }
    }

    fn set_compact(&self, compact: bool) {
        self.set_compact.set(compact);
    }

    fn select(&self, index: usize) {
        self.set_selected.set(Some(index));
        self.set_status.set(format!("Row {index} selected"));
    }

    fn show_timings(&self, shown: bool) {
        self.set_timings.set(shown);
    }
}

#[component]
fn scroll_row(index: usize, rows: Rows, compact: bool) -> NodeId {
    let is_selected = rows.selection.memo(Some(index));

    let vertical = if compact {
        COMPACT_ROW_PADDING_VERTICAL
    } else {
        ROW_PADDING_VERTICAL
    };
    let timings = rows.timings.clone();
    let value_color = {
        let is_selected = is_selected.clone();
        Prop::Dynamic(Box::new(move || {
            if is_selected.get() {
                ACCENT
            } else {
                TEXT_MUTED
            }
        }))
    };

    view! {
        <unstyled::button
            on_click={move || rows.select(index)}
            content={Box::new(move |handle: unstyled::ButtonHandle| {
                let hovered = handle.hovered;
                let fill_color = Prop::Dynamic(Box::new(move || {
                    match (is_selected.get(), hovered.get()) {
                        (true, _) => ACCENT_SOFT,
                        (false, true) => SURFACE_RAISED,
                        (false, false) => Color32::TRANSPARENT,
                    }
                }));
                view! {
                    <outline color={ACCENT} width={2.0} radius={RADIUS} offset={0.0} visible={handle.focused}>
                        <fill color={fill_color} radius={RADIUS}>
                            <padding horizontal={ROW_PADDING_HORIZONTAL} vertical={vertical}>
                                <centered_row spacing={12.0}>
                                    @percent(100.0) <body content={format!("Row {index}")} />
                                    <visibility visible={timings}>
                                        <caption
                                            content={format!("{} ms", 7 + index * 3 % 91)}
                                            align={TextAlign::End}
                                            color={value_color}
                                        />
                                    </visibility>
                                </centered_row>
                            </padding>
                        </fill>
                    </outline>
                }
            })}
        />
    }
}

#[component]
fn demo_header(set_count: WriteSignal<i64>) -> NodeId {
    let reset_count = set_count.clone();
    let decrement_count = set_count.clone();
    view! {
        <fill color={SURFACE} radius={0}>
            <padding horizontal={HEADER_PADDING} vertical={0.0}>
                <centered_row spacing={10.0}>
                    <centered_row spacing={10.0}>
                        <title content={"beui".to_string()} />
                        <caption content={"retained mode ui".to_string()} />
                    </centered_row>
                    @percent(100.0) <spacer />
                    <button label={"Reset".to_string()} variant={ButtonVariant::Secondary} on_click={move || {
                        reset_count.set(0);
                    }} />
                    @fixed(ICON_BUTTON_WIDTH) <button label={"-".to_string()} variant={ButtonVariant::Primary} on_click={move || {
                        decrement_count.update(|value| *value = value.saturating_sub(1));
                    }} />
                    @fixed(ICON_BUTTON_WIDTH) <button label={"+".to_string()} variant={ButtonVariant::Primary} on_click={move || {
                        set_count.update(|value| *value = value.saturating_add(1));
                    }} />
                </centered_row>
            </padding>
        </fill>
    }
}

#[component]
fn demo_body(count: ReadSignal<i64>) -> NodeId {
    view! {
        <padding horizontal={BODY_PADDING} vertical={BODY_PADDING}>
            <row spacing={20.0}>
                @percent(32.0) <sidebar />
                @percent(68.0) <main_panel count={count} />
            </row>
        </padding>
    }
}

#[component]
fn sidebar() -> NodeId {
    view! {
        <card>
            <column spacing={12.0}>
                <accordion title={"About".to_string()} open={true}>
                    <paragraph content={"beui keeps a retained tree of nodes. Base nodes carry behaviour only, unstyled \
                         components compose them, and the styled components paint them.".to_string()} />
                </accordion>
                @fixed(SEPARATOR_HEIGHT) <separator/>
                <accordion title={"Keyboard".to_string()} open={true}>
                    <column spacing={12.0}>
                        <shortcut keys={"Tab".to_string()} description={"move focus to the next control".to_string()} />
                        <shortcut keys={"Shift+Tab".to_string()} description={"move focus back".to_string()} />
                        <shortcut keys={"Enter".to_string()} description={"activate the focused control".to_string()} />
                        <shortcut keys={"Arrows".to_string()} description={"adjust sliders or move within choices".to_string()} />
                        <shortcut keys={"Page Up/Down".to_string()} description={"scroll the focused row list".to_string()} />
                        <shortcut keys={"Ctrl+Z".to_string()} description={"undo an edit in a text field".to_string()} />
                        <shortcut keys={"Ctrl+Shift+I".to_string()} description={"open the inspector".to_string()} />
                        <shortcut keys={"Ctrl+Shift+C".to_string()} description={"pick a node to inspect".to_string()} />
                    </column>
                </accordion>
            </column>
        </card>
    }
}

#[component]
fn main_panel(count: ReadSignal<i64>) -> NodeId {
    let (status_text, set_status_text) = create_signal("Nothing selected".to_string());
    let rows = Rows::new(set_status_text);
    let row_height = {
        let compact = rows.compact.clone();
        Prop::Dynamic(Box::new(move || {
            if compact.get() {
                COMPACT_ROW_HEIGHT
            } else {
                ROW_HEIGHT
            }
        }))
    };
    let item_rows = rows.clone();
    let (scroll_position, set_scroll_position) = create_signal(ScrollPosition::ZERO);

    view! {
        <column spacing={20.0}>
            <card>
                <column spacing={4.0}>
                    <caption content={"Counter".to_string()} />
                    <display content={create_memo(move || count.get().to_string())} />
                    <paragraph content={"Click the header buttons, or focus one with Tab and press Enter.".to_string()} />
                </column>
            </card>
            <controls rows={rows.clone()} />
            @percent(100.0) <card>
                <column spacing={12.0}>
                    <centered_row spacing={12.0}>
                        <heading content={format!("Rows ({ROW_COUNT})")} />
                        @percent(100.0) <caption content={status_text} align={TextAlign::End} />
                    </centered_row>
                    @fixed(SEPARATOR_HEIGHT) <separator/>
                    @percent(100.0) <row spacing={10.0}>
                        @percent(100.0) <virtual_list
                            count={ROW_COUNT}
                            item_height={row_height}
                            focus_color={ACCENT}
                            on_change={move |position| set_scroll_position.set(position)}
                            item={Box::new(move |index| {
                                let rows = item_rows.clone();
                                let compact = rows.compact.get();
                                view! { <scroll_row index={index} rows={rows} compact={compact} /> }
                            })}
                        />
                        @fixed(SCROLLBAR_WIDTH) <scrollbar position={scroll_position} />
                    </row>
                </column>
            </card>
        </column>
    }
}

#[component]
fn controls(rows: Rows) -> NodeId {
    let (selected_tab, set_selected_tab) = create_signal(0usize);

    let list_rows = rows.clone();
    let list_tab = selected_tab.clone();
    let load_tab = selected_tab.clone();
    let name_tab = selected_tab.clone();
    let choices_tab = selected_tab.clone();
    let list_condition = create_memo(move || list_tab.get() == 0);
    let load_condition = create_memo(move || load_tab.get() == 1);
    let name_condition = create_memo(move || name_tab.get() == 2);
    let choices_condition = create_memo(move || choices_tab.get() == 3);
    let menus_condition = create_memo(move || selected_tab.get() == 4);

    view! {
        <card>
            <column spacing={16.0}>
                <tabs labels={vec!["List".to_string(), "Load".to_string(), "Name".to_string(), "Choices".to_string(), "Menus".to_string()]} selected={0} on_change={move |selected| {
                    set_selected_tab.set(selected);
                }} />
                <column spacing={0.0}>
                    <show condition={list_condition} then={Box::new(move || view! { <list_controls rows={list_rows} /> })} />
                    <show condition={load_condition} then={Box::new(|| view! { <load_controls /> })} />
                    <show condition={name_condition} then={Box::new(|| view! { <name_controls /> })} />
                    <show condition={choices_condition} then={Box::new(|| view! { <choice_controls /> })} />
                    <show condition={menus_condition} then={Box::new(|| view! { <menu_controls /> })} />
                </column>
            </column>
        </card>
    }
}

#[component]
fn list_controls(rows: Rows) -> NodeId {
    let timing_rows = rows.clone();
    let compact_rows = rows.clone();
    view! {
        <column spacing={12.0}>
            <checkbox label={"Show timings".to_string()} checked={true} on_change={move |checked| {
                timing_rows.show_timings(checked);
            }} />
            <centered_row spacing={12.0}>
                <switch on={false} on_change={move |on| compact_rows.set_compact(on)} />
                @percent(100.0) <body content={"Compact rows".to_string()} />
            </centered_row>
        </column>
    }
}

#[component]
fn load_controls() -> NodeId {
    let (progress_value, set_progress_value) = create_signal(0.4f32);
    let readout_value = progress_value.clone();

    view! {
        <column spacing={12.0}>
            <centered_row spacing={12.0}>
                <caption content={"Simulated load".to_string()} />
                @percent(100.0) <caption content={create_memo(move || percent_label(readout_value.get()))} align={TextAlign::End} />
            </centered_row>
            <slider value={0.4} on_change={move |value| {
                set_progress_value.set(value);
            }} />
            <progress value={progress_value} />
        </column>
    }
}

#[component]
fn name_controls() -> NodeId {
    let (greeting_text, set_greeting_text) = create_signal(greeting_label(""));

    view! {
        <column spacing={12.0}>
            <centered_row spacing={12.0}>
                <caption content={"Display name".to_string()} />
                @percent(100.0) <caption content={greeting_text} align={TextAlign::End} />
            </centered_row>
            <text_input value={String::new()} placeholder={"Type a name".to_string()} on_change={move |value| {
                set_greeting_text.set(greeting_label(&value));
            }} />
            <paragraph content={"Click to place the caret, drag to select, and Ctrl+Z to undo.".to_string()} />
        </column>
    }
}

fn greeting_label(name: &str) -> String {
    if name.is_empty() {
        "Nobody yet".to_owned()
    } else {
        format!("Hello, {name}")
    }
}

fn percent_label(value: f32) -> String {
    format!("{}%", (value * 100.0).round())
}

#[component]
fn choice_controls() -> NodeId {
    let modes = ["Automatic", "Manual", "Scheduled"];
    let (mode_status_text, set_mode_status_text) = create_signal("Automatic updates".to_string());
    let colors = ["Amber", "Blue", "Green", "Purple"];
    let (color_status_text, set_color_status_text) = create_signal("Blue selected".to_string());
    let (pin_status_text, set_pin_status_text) = create_signal("Selection is unpinned".to_string());

    view! {
        <row spacing={20.0}>
            @percent(50.0) <column spacing={8.0}>
                <caption content={"Update mode".to_string()} />
                <radio_group labels={vec!["Automatic".to_string(), "Manual".to_string(), "Scheduled".to_string()]} selected={Some(0)} on_change={move |selected| {
                    if let Some(index) = selected {
                        let text = format!("{} updates", modes[index]);
                        set_mode_status_text.set(text);
                    }
                }} />
                <caption content={mode_status_text} />
                <toggle_button label={"Pin selection".to_string()} pressed={false} on_change={move |pressed| {
                    let text = if pressed {
                        "Selection is pinned"
                    } else {
                        "Selection is unpinned"
                    }
                    .to_string();
                    set_pin_status_text.set(text);
                }} />
                <caption content={pin_status_text} />
            </column>
            @percent(50.0) <column spacing={8.0}>
                <caption content={"Highlight color (type to search)".to_string()} />
                <listbox labels={vec!["Amber".to_string(), "Blue".to_string(), "Green".to_string(), "Purple".to_string()]} selected={Some(1)} on_change={move |selected| {
                    if let Some(index) = selected {
                        let text = format!("{} selected", colors[index]);
                        set_color_status_text.set(text);
                    }
                }} />
                <caption content={color_status_text} />
            </column>
        </row>
    }
}

#[component]
fn menu_controls() -> NodeId {
    let fruits: Vec<String> = ["Apple", "Banana", "Cherry", "Date", "Grape", "Mango"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let fruit_names = fruits.clone();
    let (fruit_status_text, set_fruit_status_text) = create_signal("Apple selected".to_string());
    let (menu_status_text, set_menu_status_text) = create_signal("Nothing chosen yet".to_string());

    let items = vec![
        unstyled::MenuItem::new("Copy"),
        unstyled::MenuItem::new("Paste"),
        unstyled::MenuItem::with_children(
            "Share",
            vec![
                unstyled::MenuItem::new("Email"),
                unstyled::MenuItem::new("Link"),
            ],
        ),
    ];

    view! {
        <row spacing={20.0}>
            @percent(50.0) <column spacing={8.0}>
                <caption content={"Favorite fruit (type to search)".to_string()} />
                <select options={fruits} selected={Some(0)} on_change={move |selected| {
                    let text = selected
                        .and_then(|index| fruit_names.get(index))
                        .map_or_else(
                            || "Nothing selected".to_owned(),
                            |label| format!("{label} selected"),
                        );
                    set_fruit_status_text.set(text);
                }} />
                <caption content={fruit_status_text} />
            </column>
            @percent(50.0) <column spacing={8.0}>
                <context_menu region={
                    view! {
                        <card>
                            <column spacing={4.0}>
                                <caption content={"Right-click the card below".to_string()} />
                                <paragraph content={"The Share item opens a submenu on hover or Right Arrow; Left Arrow closes it.".to_string()} />
                            </column>
                        </card>
                    }
                } items={items} on_select={move |path: Vec<usize>| {
                    let label = match path.as_slice() {
                        [0] => "Copy".to_owned(),
                        [1] => "Paste".to_owned(),
                        [2, 0] => "Share > Email".to_owned(),
                        [2, 1] => "Share > Link".to_owned(),
                        other => format!("{other:?}"),
                    };
                    set_menu_status_text.set(format!("Chose: {label}"));
                }} />
                <caption content={menu_status_text} />
            </column>
        </row>
    }
}
