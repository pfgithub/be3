use beui::reactive::{
    build, clone, create_memo, create_selector, create_signal, view, CenteredRowBuilder,
    ColumnBuilder, FillBuilder, Memo, OutlineBuilder, PaddingBuilder, ReadSignal, RowBuilder,
    Selector, ShowBuilder, SpacerBuilder, VirtualListBuilder, VisibilityBuilder, WriteSignal,
};
use beui::styled::theme::{
    ACCENT, ACCENT_SOFT, BACKGROUND, NARROW_WIDTH, RADIUS, SCROLLBAR_WIDTH, SEPARATOR_HEIGHT,
    SURFACE, SURFACE_RAISED, TEXT_MUTED,
};
use beui::styled::{
    AccordionBuilder, BodyBuilder, ButtonBuilder, ButtonVariant, CaptionBuilder, CardBuilder,
    CheckboxBuilder, ContextMenuBuilder, DisplayBuilder, HeadingBuilder, ListboxBuilder,
    ParagraphBuilder, ProgressBuilder, RadioGroupBuilder, ResponsiveTabsBuilder, ScrollbarBuilder,
    SelectBuilder, SeparatorBuilder, ShortcutBuilder, SliderBuilder, StackBuilder, SwitchBuilder,
    TextInputBuilder, TitleBuilder, ToggleButtonBuilder,
};
use beui::unstyled::{narrower_than, ContainerBuilder};
use beui::{
    unstyled, Color32, Context, Document, ItemSize, NodeId, Rect, ScrollPosition, TextAlign,
};
use beui_macros::component;
use std::cell::Cell;
use std::rc::Rc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("beui demo", DemoApp::new())
}

const HEADER_HEIGHT: f32 = 64.0;
const COMPACT_HEADER_HEIGHT: f32 = 52.0;
const HEADER_PADDING: f32 = 20.0;
const BODY_PADDING: f32 = 20.0;
const COMPACT_PADDING: f32 = 12.0;
const BODY_SPACING: f32 = 20.0;
const CARD_NARROW_WIDTH: f32 = 460.0;
const TABS_NARROW_WIDTH: f32 = 380.0;
const ICON_BUTTON_WIDTH: f32 = 44.0;
const ROW_COUNT: usize = 10_000;
const NARROW_ROWS_HEIGHT: f32 = 320.0;
const ROW_HEIGHT: f32 = 34.0;
const COMPACT_ROW_HEIGHT: f32 = 25.0;
const ROW_PADDING_HORIZONTAL: f32 = 12.0;
const ROW_PADDING_VERTICAL: f32 = 9.0;
const COMPACT_ROW_PADDING_VERTICAL: f32 = 4.0;

struct DemoApp {
    document: Document,
    emulate_touch: Rc<Cell<bool>>,
}

impl DemoApp {
    fn new() -> Self {
        let emulate_touch = Rc::new(Cell::new(false));
        let touch_setting = emulate_touch.clone();
        let document = build(|| {
            let (count, set_count) = create_signal(0i64);
            view! {
                <fill color=BACKGROUND radius=0>
                    <container>
                        {move |_| view! { <demo_shell count set_count emulate_touch={touch_setting} /> }}
                    </container>
                </fill>
            }
        });

        Self {
            document,
            emulate_touch,
        }
    }
}

impl beui::App for DemoApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        BACKGROUND
    }

    fn emulate_touch_with_mouse(&self) -> bool {
        self.emulate_touch.get()
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
        let selection = create_selector(clone!(selected -> move || selected.get()));
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
    let selected = rows.selection.memo(Some(index));
    let select_rows = rows.clone();
    view! {
        <unstyled::button
            on_click={move || select_rows.select(index)}
            content={move |handle| view! {
                <scroll_row_face
                    index
                    handle
                    selected
                    timings={rows.timings}
                    compact
                />
            }}
        />
    }
}

#[component]
fn scroll_row_face(
    index: usize,
    handle: unstyled::ButtonHandle,
    selected: Memo<bool>,
    timings: ReadSignal<bool>,
    compact: bool,
) -> NodeId {
    let unstyled::ButtonHandle {
        hovered, focused, ..
    } = handle;
    let vertical = if compact {
        COMPACT_ROW_PADDING_VERTICAL
    } else {
        ROW_PADDING_VERTICAL
    };
    let value_color =
        create_memo(clone!(selected -> move || if selected.get() { ACCENT } else { TEXT_MUTED }));
    let fill_color = create_memo(move || match (selected.get(), hovered.get()) {
        (true, _) => ACCENT_SOFT,
        (false, true) => SURFACE_RAISED,
        (false, false) => Color32::TRANSPARENT,
    });

    view! {
        <outline color=ACCENT width=2.0 radius=RADIUS offset=0.0 visible={focused}>
            <fill color={fill_color} radius=RADIUS>
                <padding horizontal=ROW_PADDING_HORIZONTAL vertical>
                    <centered_row spacing=12.0>
                        <body @sizing=ItemSize::Percent(100.0) content={format!("Row {index}")} />
                        <visibility visible={timings}>
                            <caption
                                content={format!("{} ms", 7 + index * 3 % 91)}
                                align=TextAlign::End
                                color={value_color}
                            />
                        </visibility>
                    </centered_row>
                </padding>
            </fill>
        </outline>
    }
}

#[component]
fn demo_shell(
    count: ReadSignal<i64>,
    set_count: WriteSignal<i64>,
    emulate_touch: Rc<Cell<bool>>,
) -> NodeId {
    let narrow = narrower_than(NARROW_WIDTH);
    let header_height = create_memo(move || {
        if narrow.get() {
            ItemSize::Fixed(COMPACT_HEADER_HEIGHT)
        } else {
            ItemSize::Fixed(HEADER_HEIGHT)
        }
    });
    view! {
        <column spacing=0.0>
            <demo_header @sizing={header_height} set_count />
            <separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
            <demo_body @sizing=ItemSize::Percent(100.0) count emulate_touch />
        </column>
    }
}

#[component]
fn demo_header(set_count: WriteSignal<i64>) -> NodeId {
    let reset_count = set_count.clone();
    let decrement_count = set_count.clone();
    let narrow = narrower_than(NARROW_WIDTH);
    let wide = create_memo(clone!(narrow -> move || !narrow.get()));
    let horizontal = create_memo(move || {
        if narrow.get() {
            COMPACT_PADDING
        } else {
            HEADER_PADDING
        }
    });
    view! {
        <fill color=SURFACE radius=0>
            <padding horizontal vertical=0.0>
                <centered_row spacing=10.0>
                    <centered_row spacing=10.0>
                        <title content="beui" />
                        <visibility visible={wide}>
                            <caption content="retained mode ui" />
                        </visibility>
                    </centered_row>
                    <spacer @sizing=ItemSize::Percent(100.0) />
                    <button label="Reset" variant=ButtonVariant::Secondary on_click={move || {
                        reset_count.set(0);
                    }} />
                    <button @sizing=ItemSize::Fixed(ICON_BUTTON_WIDTH) label="-" variant=ButtonVariant::Primary on_click={move || {
                        decrement_count.update(|value| *value = value.saturating_sub(1));
                    }} />
                    <button @sizing=ItemSize::Fixed(ICON_BUTTON_WIDTH) label="+" variant=ButtonVariant::Primary on_click={move || {
                        set_count.update(|value| *value = value.saturating_add(1));
                    }} />
                </centered_row>
            </padding>
        </fill>
    }
}

#[component]
fn demo_body(count: ReadSignal<i64>, emulate_touch: Rc<Cell<bool>>) -> NodeId {
    let narrow = narrower_than(NARROW_WIDTH);
    let padding = create_memo(move || {
        if narrow.get() {
            COMPACT_PADDING
        } else {
            BODY_PADDING
        }
    });
    view! {
        <padding horizontal={padding.clone()} vertical={padding}>
            <stack spacing=BODY_SPACING>
                <sidebar @sizing=ItemSize::Percent(32.0) emulate_touch />
                <main_panel @sizing=ItemSize::Percent(68.0) count />
            </stack>
        </padding>
    }
}

#[component]
fn sidebar(emulate_touch: Rc<Cell<bool>>) -> NodeId {
    let narrow = narrower_than(NARROW_WIDTH);
    let open = create_memo(move || !narrow.get());
    let keyboard_open = open.clone();
    view! {
        <card>
            <column spacing=12.0>
                <accordion title="About" open>
                    <paragraph content="beui keeps a retained tree of nodes. Base nodes carry behaviour only, unstyled \
                         components compose them, and the styled components paint them." />
                </accordion>
                <separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                <accordion title="Touch testing" open=false>
                    <checkbox label="Emulate touch with mouse" checked=false on_change={move |enabled| {
                        emulate_touch.set(enabled);
                    }} />
                </accordion>
                <separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                <accordion title="Keyboard" open={keyboard_open}>
                    <column spacing=12.0>
                        <shortcut keys="Tab" description="move focus to the next control" />
                        <shortcut keys="Shift+Tab" description="move focus back" />
                        <shortcut keys="Enter" description="activate the focused control" />
                        <shortcut keys="Arrows" description="adjust sliders or move within choices" />
                        <shortcut keys="Page Up/Down" description="scroll the focused row list" />
                        <shortcut keys="Ctrl+Z" description="undo an edit in a text field" />
                        <shortcut keys="Ctrl+Shift+I" description="open the inspector" />
                        <shortcut keys="Ctrl+Shift+C" description="pick a node to inspect" />
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
    let compact = rows.compact.clone();
    let row_height = create_memo(move || {
        if compact.get() {
            COMPACT_ROW_HEIGHT
        } else {
            ROW_HEIGHT
        }
    });
    let item_rows = rows.clone();
    let (scroll_position, set_scroll_position) = create_signal(ScrollPosition::ZERO);
    let narrow = narrower_than(NARROW_WIDTH);
    let rows_size = create_memo(move || {
        if narrow.get() {
            ItemSize::Fixed(NARROW_ROWS_HEIGHT)
        } else {
            ItemSize::Percent(100.0)
        }
    });

    view! {
        <column spacing=20.0>
            <card>
                <column spacing=4.0>
                    <caption content="Counter" />
                    <display content={create_memo(clone!(count -> move || count.get().to_string()))} />
                    <paragraph content="Click the header buttons, or focus one with Tab and press Enter." />
                </column>
            </card>
            <controls rows={rows.clone()} />
            <card @sizing={rows_size}>
                <column spacing=12.0>
                    <centered_row spacing=12.0>
                        <heading content={format!("Rows ({ROW_COUNT})")} />
                        <caption @sizing=ItemSize::Percent(100.0) content={status_text} align=TextAlign::End />
                    </centered_row>
                    <separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <row @sizing=ItemSize::Percent(100.0) spacing=10.0>
                        <virtual_list @sizing=ItemSize::Percent(100.0)
                            count=ROW_COUNT
                            item_height={row_height}
                            focus_color=ACCENT
                            on_change={move |position| set_scroll_position.set(position)}
                        >
                            {move |index: usize| {
                                let rows = item_rows.clone();
                                let compact = rows.compact.get();
                                view! { <scroll_row index rows compact /> }
                            }}
                        </virtual_list>
                        <scrollbar @sizing=ItemSize::Fixed(SCROLLBAR_WIDTH) position={scroll_position} />
                    </row>
                </column>
            </card>
        </column>
    }
}

#[component]
fn controls(rows: Rows) -> NodeId {
    view! {
        <card>
            <container>{move |_| view! { <control_panels rows /> }}</container>
        </card>
    }
}

#[component]
fn control_panels(rows: Rows) -> NodeId {
    let (selected_tab, set_selected_tab) = create_signal(0usize);

    let tab = create_selector(clone!(selected_tab -> move || selected_tab.get()));
    let list_rows = rows.clone();

    view! {
        <column spacing=16.0>
            <responsive_tabs
                labels={vec!["List".to_string(), "Load".to_string(), "Name".to_string(), "Choices".to_string(), "Menus".to_string()]}
                selected=0
                breakpoint=TABS_NARROW_WIDTH
                on_change={move |selected| {
                    set_selected_tab.set(selected);
                }}
            />
            <column spacing=0.0>
                <show condition={tab.memo(0)}><list_controls rows=list_rows /></show>
                <show condition={tab.memo(1)}><load_controls /></show>
                <show condition={tab.memo(2)}><name_controls /></show>
                <show condition={tab.memo(3)}><choice_controls /></show>
                <show condition={tab.memo(4)}><menu_controls /></show>
            </column>
        </column>
    }
}

#[component]
fn list_controls(rows: Rows) -> NodeId {
    let timing_rows = rows.clone();
    let compact_rows = rows.clone();
    view! {
        <column spacing=12.0>
            <checkbox label="Show timings" checked=true on_change={move |checked| {
                timing_rows.show_timings(checked);
            }} />
            <centered_row spacing=12.0>
                <switch on=false on_change={move |on| compact_rows.set_compact(on)} />
                <body @sizing=ItemSize::Percent(100.0) content="Compact rows" />
            </centered_row>
        </column>
    }
}

#[component]
fn load_controls() -> NodeId {
    let (progress_value, set_progress_value) = create_signal(0.4f32);
    let readout_value = progress_value.clone();

    view! {
        <column spacing=12.0>
            <centered_row spacing=12.0>
                <caption content="Simulated load" />
                <caption @sizing=ItemSize::Percent(100.0) content={create_memo(move || percent_label(readout_value.get()))} align=TextAlign::End />
            </centered_row>
            <slider value=0.4 on_change={move |value| {
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
        <column spacing=12.0>
            <centered_row spacing=12.0>
                <caption content="Display name" />
                <caption @sizing=ItemSize::Percent(100.0) content={greeting_text} align=TextAlign::End />
            </centered_row>
            <text_input value=String::new() placeholder="Type a name" on_change={move |value| {
                set_greeting_text.set(greeting_label(&value));
            }} />
            <paragraph content="Click to place the caret, drag to select, and Ctrl+Z to undo." />
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
        <stack spacing=20.0 breakpoint=CARD_NARROW_WIDTH>
            <column @sizing=ItemSize::Percent(50.0) spacing=8.0>
                <caption content="Update mode" />
                <radio_group labels={vec!["Automatic".to_string(), "Manual".to_string(), "Scheduled".to_string()]} selected=Some(0) on_change={move |selected| {
                    if let Some(index) = selected {
                        let text = format!("{} updates", modes[index]);
                        set_mode_status_text.set(text);
                    }
                }} />
                <caption content={mode_status_text} />
                <toggle_button label="Pin selection" pressed=false on_change={move |pressed| {
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
            <column @sizing=ItemSize::Percent(50.0) spacing=8.0>
                <caption content="Highlight color (type to search)" />
                <listbox labels={vec!["Amber".to_string(), "Blue".to_string(), "Green".to_string(), "Purple".to_string()]} selected=Some(1) on_change={move |selected| {
                    if let Some(index) = selected {
                        let text = format!("{} selected", colors[index]);
                        set_color_status_text.set(text);
                    }
                }} />
                <caption content={color_status_text} />
            </column>
        </stack>
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
        <stack spacing=20.0 breakpoint=CARD_NARROW_WIDTH>
            <column @sizing=ItemSize::Percent(50.0) spacing=8.0>
                <caption content="Favorite fruit (type to search)" />
                <select options={fruits} selected=Some(0) on_change={move |selected| {
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
            <column @sizing=ItemSize::Percent(50.0) spacing=8.0>
                <context_menu items on_select={move |path: Vec<usize>| {
                    let label = match path.as_slice() {
                        [0] => "Copy".to_owned(),
                        [1] => "Paste".to_owned(),
                        [2, 0] => "Share > Email".to_owned(),
                        [2, 1] => "Share > Link".to_owned(),
                        other => format!("{other:?}"),
                    };
                    set_menu_status_text.set(format!("Chose: {label}"));
                }}>
                    <card>
                        <column spacing=4.0>
                            <caption content="Right-click the card below" />
                            <paragraph content="The Share item opens a submenu on hover or Right Arrow; Left Arrow closes it." />
                        </column>
                    </card>
                </context_menu>
                <caption content={menu_status_text} />
            </column>
        </stack>
    }
}
