use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use beui::reactive::{create_memo, create_signal, view, with_reactive_scope, WriteSignal};
use beui::styled::theme::{
    ACCENT, ACCENT_SOFT, BACKGROUND, RADIUS, SCROLLBAR_WIDTH, SEPARATOR_HEIGHT, SURFACE,
    SURFACE_RAISED, TEXT_MUTED,
};
use beui::styled::{
    self, AccordionBuilder, BodyBuilder, ButtonBuilder, ButtonVariant, CaptionBuilder, CardBuilder,
    CheckboxBuilder, DisplayBuilder, HeadingBuilder, ListboxBuilder, ParagraphBuilder,
    ProgressBuilder, RadioGroupBuilder, ScrollbarBuilder, ShortcutBuilder, SliderBuilder,
    SwitchBuilder, TabsBuilder, TitleBuilder, ToggleButtonBuilder,
};
use beui::{unstyled, Color32, Context, Document, ItemSize, NodeId, Rect, TextAlign};

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
        let value = with_reactive_scope(&mut document, || {
            view! { <display content={create_memo(move || count.get().to_string())} /> }
        });

        let header = build_header(&mut document, set_count);
        let body = build_body(&mut document, value);

        let root_column = unstyled::column(&mut document, 0.0);
        document.append_child(root_column, header, ItemSize::Fixed(HEADER_HEIGHT));
        let header_line = styled::separator(&mut document);
        document.append_child(root_column, header_line, ItemSize::Fixed(SEPARATOR_HEIGHT));
        document.append_child(root_column, body, ItemSize::Percent(100.0));

        let background = document.create_fill(BACKGROUND, 0);
        document.set_fill_child(background, root_column);
        document.set_root(background);

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

struct Rows {
    set_status: WriteSignal<String>,
    selected: Cell<Option<usize>>,
    timings: Cell<bool>,
    visuals: RefCell<HashMap<usize, Weak<RowVisual>>>,
}

impl Rows {
    fn new(set_status: WriteSignal<String>) -> Self {
        Self {
            set_status,
            selected: Cell::new(None),
            timings: Cell::new(true),
            visuals: RefCell::new(HashMap::new()),
        }
    }

    fn register(&self, visual: &Rc<RowVisual>) {
        let mut visuals = self.visuals.borrow_mut();
        visuals.retain(|_, visual| visual.strong_count() > 0);
        visuals.insert(visual.index, Rc::downgrade(visual));
    }

    fn visual(&self, index: usize) -> Option<Rc<RowVisual>> {
        self.visuals.borrow().get(&index)?.upgrade()
    }

    fn live(&self) -> Vec<Rc<RowVisual>> {
        self.visuals
            .borrow()
            .values()
            .filter_map(Weak::upgrade)
            .collect()
    }

    fn select(&self, document: &mut Document, index: usize) {
        let previous = self.selected.replace(Some(index));
        if let Some(visual) = previous.and_then(|previous| self.visual(previous)) {
            visual.apply(document);
        }
        if let Some(visual) = self.visual(index) {
            visual.apply(document);
        }
        let set_status = self.set_status.clone();
        with_reactive_scope(document, || set_status.set(format!("Row {index} selected")));
    }

    fn show_timings(&self, document: &mut Document, shown: bool) {
        self.timings.set(shown);
        for visual in self.live() {
            visual.apply(document);
        }
    }
}

struct RowVisual {
    index: usize,
    fill: NodeId,
    value: NodeId,
    timing: NodeId,
    hovered: Cell<bool>,
    rows: Rc<Rows>,
}

impl RowVisual {
    fn selected(&self) -> bool {
        self.rows.selected.get() == Some(self.index)
    }

    fn apply(&self, document: &mut Document) {
        let fill = match (self.selected(), self.hovered.get()) {
            (true, _) => ACCENT_SOFT,
            (false, true) => SURFACE_RAISED,
            (false, false) => Color32::TRANSPARENT,
        };
        document.set_fill_color(self.fill, fill);
        let value = if self.selected() { ACCENT } else { TEXT_MUTED };
        document.set_text_color(self.value, value);
        document.set_visible(self.timing, self.rows.timings.get());
    }
}

fn scroll_row(document: &mut Document, index: usize, rows: &Rc<Rows>, compact: bool) -> NodeId {
    let (label, value) = with_reactive_scope(document, || {
        let label = view! { <body content={format!("Row {index}")} /> };
        let value = view! { <caption content={format!("{} ms", 7 + index * 3 % 91)} /> };
        (label, value)
    });
    let value_text = document.shadow_root(value);
    document.set_text_align(value_text, TextAlign::End, TextAlign::Center);
    let timing = document.create_visibility(rows.timings.get());
    document.set_visibility_child(timing, value);

    let line = unstyled::centered_row(document, 12.0);
    document.append_child(line, label, ItemSize::Percent(100.0));
    document.append_child(line, timing, ItemSize::Intrinsic);

    let vertical = if compact {
        COMPACT_ROW_PADDING_VERTICAL
    } else {
        ROW_PADDING_VERTICAL
    };
    let padding = document.create_padding(ROW_PADDING_HORIZONTAL, vertical);
    document.set_padding_child(padding, line);

    let fill = document.create_fill(Color32::TRANSPARENT, RADIUS);
    document.set_fill_child(fill, padding);

    let ring = document.create_outline(ACCENT, 2.0, RADIUS, 0.0);
    document.set_outline_child(ring, fill);
    let catcher = unstyled::button(document);
    unstyled::set_button_child(document, catcher, ring);
    unstyled::set_button_on_focus_change(document, catcher, move |document, focused| {
        document.set_outline_visible(ring, focused);
    });

    let visual = Rc::new(RowVisual {
        index,
        fill,
        value: value_text,
        timing,
        hovered: Cell::new(false),
        rows: rows.clone(),
    });
    rows.register(&visual);
    visual.apply(document);

    let hovered = visual.clone();
    unstyled::set_button_on_hover_change(document, catcher, move |document, is_hovered| {
        hovered.hovered.set(is_hovered);
        hovered.apply(document);
    });

    let clicked = visual;
    unstyled::set_button_on_click(document, catcher, move |document| {
        clicked.rows.select(document, clicked.index);
    });

    catcher
}

fn install_rows(document: &mut Document, scroll: NodeId, rows: &Rc<Rows>, compact: bool) {
    let height = if compact {
        COMPACT_ROW_HEIGHT
    } else {
        ROW_HEIGHT
    };
    let rows = rows.clone();
    document.set_scroll_virtual_items(scroll, ROW_COUNT, height, move |document, index| {
        scroll_row(document, index, &rows, compact)
    });
}

fn build_header(document: &mut Document, set_count: WriteSignal<i64>) -> NodeId {
    let title = with_reactive_scope(
        document,
        || view! { <title content={"beui".to_string()} /> },
    );
    let subtitle = with_reactive_scope(document, || {
        view! { <caption content={"retained mode ui".to_string()} /> }
    });
    let brand = unstyled::centered_row(document, 10.0);
    document.append_child(brand, title, ItemSize::Intrinsic);
    document.append_child(brand, subtitle, ItemSize::Intrinsic);

    let reset = with_reactive_scope(document, || {
        view! { <button label={"Reset".to_string()} variant={ButtonVariant::Secondary} /> }
    });
    let decrement = with_reactive_scope(document, || {
        view! { <button label={"-".to_string()} variant={ButtonVariant::Primary} /> }
    });
    let increment = with_reactive_scope(document, || {
        view! { <button label={"+".to_string()} variant={ButtonVariant::Primary} /> }
    });

    styled::set_button_on_click(document, reset, {
        let set_count = set_count.clone();
        move |_document| set_count.set(0)
    });
    styled::set_button_on_click(document, decrement, {
        let set_count = set_count.clone();
        move |_document| set_count.update(|value| *value = value.saturating_sub(1))
    });
    styled::set_button_on_click(document, increment, move |_document| {
        set_count.update(|value| *value = value.saturating_add(1))
    });

    let gap = unstyled::spacer(document);
    let bar = unstyled::centered_row(document, 10.0);
    document.append_child(bar, brand, ItemSize::Intrinsic);
    document.append_child(bar, gap, ItemSize::Percent(100.0));
    document.append_child(bar, reset, ItemSize::Intrinsic);
    document.append_child(bar, decrement, ItemSize::Fixed(ICON_BUTTON_WIDTH));
    document.append_child(bar, increment, ItemSize::Fixed(ICON_BUTTON_WIDTH));

    let padding = document.create_padding(HEADER_PADDING, 0.0);
    document.set_padding_child(padding, bar);
    let fill = document.create_fill(SURFACE, 0);
    document.set_fill_child(fill, padding);
    fill
}

fn build_body(document: &mut Document, value: NodeId) -> NodeId {
    let panes = unstyled::row(document, 20.0);
    let sidebar = build_sidebar(document);
    let main = build_main(document, value);
    document.append_child(panes, sidebar, ItemSize::Percent(32.0));
    document.append_child(panes, main, ItemSize::Percent(68.0));

    let padding = document.create_padding(BODY_PADDING, BODY_PADDING);
    document.set_padding_child(padding, panes);
    padding
}

fn build_sidebar(document: &mut Document) -> NodeId {
    let about = with_reactive_scope(document, || {
        view! {
            <paragraph content={"beui keeps a retained tree of nodes. Base nodes carry behaviour only, unstyled \
                 components compose them, and the styled components paint them.".to_string()} />
        }
    });
    let about_section = with_reactive_scope(document, || {
        view! { <accordion title={"About".to_string()} open={true}>{about}</accordion> }
    });

    let line = styled::separator(document);

    let (tab, shift_tab, enter, arrows, wheel, typing, inspect, pick) = with_reactive_scope(
        document,
        || {
            (
                view! { <shortcut keys={"Tab".to_string()} description={"move focus to the next control".to_string()} /> },
                view! { <shortcut keys={"Shift+Tab".to_string()} description={"move focus back".to_string()} /> },
                view! { <shortcut keys={"Enter".to_string()} description={"activate the focused control".to_string()} /> },
                view! { <shortcut keys={"Arrows".to_string()} description={"adjust sliders or move within choices".to_string()} /> },
                view! { <shortcut keys={"Page Up/Down".to_string()} description={"scroll the focused row list".to_string()} /> },
                view! { <shortcut keys={"Ctrl+Z".to_string()} description={"undo an edit in a text field".to_string()} /> },
                view! { <shortcut keys={"Ctrl+Shift+I".to_string()} description={"open the inspector".to_string()} /> },
                view! { <shortcut keys={"Ctrl+Shift+C".to_string()} description={"pick a node to inspect".to_string()} /> },
            )
        },
    );

    let keys = unstyled::column(document, 12.0);
    document.append_child(keys, tab, ItemSize::Intrinsic);
    document.append_child(keys, shift_tab, ItemSize::Intrinsic);
    document.append_child(keys, enter, ItemSize::Intrinsic);
    document.append_child(keys, arrows, ItemSize::Intrinsic);
    document.append_child(keys, wheel, ItemSize::Intrinsic);
    document.append_child(keys, typing, ItemSize::Intrinsic);
    document.append_child(keys, inspect, ItemSize::Intrinsic);
    document.append_child(keys, pick, ItemSize::Intrinsic);
    let keyboard_section = with_reactive_scope(document, || {
        view! { <accordion title={"Keyboard".to_string()} open={true}>{keys}</accordion> }
    });

    let content = unstyled::column(document, 12.0);
    document.append_child(content, about_section, ItemSize::Intrinsic);
    document.append_child(content, line, ItemSize::Fixed(SEPARATOR_HEIGHT));
    document.append_child(content, keyboard_section, ItemSize::Intrinsic);

    with_reactive_scope(document, || view! { <card>{content}</card> })
}

fn build_main(document: &mut Document, value: NodeId) -> NodeId {
    let counter_label = with_reactive_scope(document, || {
        view! { <caption content={"Counter".to_string()} /> }
    });
    let counter_hint = with_reactive_scope(document, || {
        view! { <paragraph content={"Click the header buttons, or focus one with Tab and press Enter.".to_string()} /> }
    });
    let counter_column = unstyled::column(document, 4.0);
    document.append_child(counter_column, counter_label, ItemSize::Intrinsic);
    document.append_child(counter_column, value, ItemSize::Intrinsic);
    document.append_child(counter_column, counter_hint, ItemSize::Intrinsic);
    let counter_card = with_reactive_scope(document, || view! { <card>{counter_column}</card> });

    let list_title = with_reactive_scope(document, || {
        view! { <heading content={format!("Rows ({ROW_COUNT})")} /> }
    });
    let (status_text, set_status_text) = create_signal("Nothing selected".to_string());
    let status = with_reactive_scope(document, || view! { <caption content={status_text} /> });
    let status_text_node = document.shadow_root(status);
    document.set_text_align(status_text_node, TextAlign::End, TextAlign::Center);
    let list_header = unstyled::centered_row(document, 12.0);
    document.append_child(list_header, list_title, ItemSize::Intrinsic);
    document.append_child(list_header, status, ItemSize::Percent(100.0));

    let list_line = styled::separator(document);

    let scroll = document.create_scroll();
    let rows = Rc::new(Rows::new(set_status_text));
    install_rows(document, scroll, &rows, false);
    let bar = with_reactive_scope(document, || view! { <scrollbar scroll={scroll} /> });

    let area = unstyled::row(document, 10.0);
    document.append_child(area, scroll, ItemSize::Percent(100.0));
    document.append_child(area, bar, ItemSize::Fixed(SCROLLBAR_WIDTH));

    let list_column = unstyled::column(document, 12.0);
    document.append_child(list_column, list_header, ItemSize::Intrinsic);
    document.append_child(list_column, list_line, ItemSize::Fixed(SEPARATOR_HEIGHT));
    document.append_child(list_column, area, ItemSize::Percent(100.0));
    let list_card = with_reactive_scope(document, || view! { <card>{list_column}</card> });

    let controls_card = build_controls(document, scroll, &rows);

    let main = unstyled::column(document, 20.0);
    document.append_child(main, counter_card, ItemSize::Intrinsic);
    document.append_child(main, controls_card, ItemSize::Intrinsic);
    document.append_child(main, list_card, ItemSize::Percent(100.0));
    main
}

fn build_controls(document: &mut Document, scroll: NodeId, rows: &Rc<Rows>) -> NodeId {
    let list_panel = build_list_controls(document, scroll, rows);
    let list_visibility = document.create_visibility(true);
    document.set_visibility_child(list_visibility, list_panel);

    let load_panel = build_load_controls(document);
    let load_visibility = document.create_visibility(false);
    document.set_visibility_child(load_visibility, load_panel);

    let name_panel = build_name_controls(document);
    let name_visibility = document.create_visibility(false);
    document.set_visibility_child(name_visibility, name_panel);

    let choices_panel = build_choice_controls(document);
    let choices_visibility = document.create_visibility(false);
    document.set_visibility_child(choices_visibility, choices_panel);

    let menus_panel = build_menu_controls(document);
    let menus_visibility = document.create_visibility(false);
    document.set_visibility_child(menus_visibility, menus_panel);

    let panels = unstyled::column(document, 0.0);
    document.append_child(panels, list_visibility, ItemSize::Intrinsic);
    document.append_child(panels, load_visibility, ItemSize::Intrinsic);
    document.append_child(panels, name_visibility, ItemSize::Intrinsic);
    document.append_child(panels, choices_visibility, ItemSize::Intrinsic);
    document.append_child(panels, menus_visibility, ItemSize::Intrinsic);

    let tabs = with_reactive_scope(document, || {
        view! {
            <tabs labels={vec!["List".to_string(), "Load".to_string(), "Name".to_string(), "Choices".to_string(), "Menus".to_string()]} selected={0} on_change={Box::new(move |document: &mut Document, selected| {
                document.set_visible(list_visibility, selected == 0);
                document.set_visible(load_visibility, selected == 1);
                document.set_visible(name_visibility, selected == 2);
                document.set_visible(choices_visibility, selected == 3);
                document.set_visible(menus_visibility, selected == 4);
            })} />
        }
    });

    let column = unstyled::column(document, 16.0);
    document.append_child(column, tabs, ItemSize::Intrinsic);
    document.append_child(column, panels, ItemSize::Intrinsic);
    with_reactive_scope(document, || view! { <card>{column}</card> })
}

fn build_list_controls(document: &mut Document, scroll: NodeId, rows: &Rc<Rows>) -> NodeId {
    let timing_rows = rows.clone();
    let timings = with_reactive_scope(document, || {
        view! {
            <checkbox label={"Show timings".to_string()} checked={true} on_change={Box::new(move |document: &mut Document, checked| {
                timing_rows.show_timings(document, checked);
            })} />
        }
    });

    let compact_rows = rows.clone();
    let (compact, compact_label) = with_reactive_scope(document, || {
        (
            view! { <switch on={false} on_change={Box::new(move |document: &mut Document, on| {
                install_rows(document, scroll, &compact_rows, on);
            })} /> },
            view! { <body content={"Compact rows".to_string()} /> },
        )
    });
    let compact_line = unstyled::centered_row(document, 12.0);
    document.append_child(compact_line, compact, ItemSize::Intrinsic);
    document.append_child(compact_line, compact_label, ItemSize::Percent(100.0));

    let column = unstyled::column(document, 12.0);
    document.append_child(column, timings, ItemSize::Intrinsic);
    document.append_child(column, compact_line, ItemSize::Intrinsic);
    column
}

fn build_load_controls(document: &mut Document) -> NodeId {
    let (progress_value, set_progress_value) = create_signal(0.4f32);
    let (label, readout, bar, slider) = with_reactive_scope(document, || {
        let readout_value = progress_value.clone();
        (
            view! { <caption content={"Simulated load".to_string()} /> },
            view! { <caption content={create_memo(move || percent_label(readout_value.get()))} /> },
            view! { <progress value={progress_value} /> },
            view! { <slider value={0.4} on_change={Box::new(move |document: &mut Document, value| {
                with_reactive_scope(document, || set_progress_value.set(value));
            })} /> },
        )
    });
    let readout_text_node = document.shadow_root(readout);
    document.set_text_align(readout_text_node, TextAlign::End, TextAlign::Center);
    let header = unstyled::centered_row(document, 12.0);
    document.append_child(header, label, ItemSize::Intrinsic);
    document.append_child(header, readout, ItemSize::Percent(100.0));

    let column = unstyled::column(document, 12.0);
    document.append_child(column, header, ItemSize::Intrinsic);
    document.append_child(column, slider, ItemSize::Intrinsic);
    document.append_child(column, bar, ItemSize::Intrinsic);
    column
}

fn build_name_controls(document: &mut Document) -> NodeId {
    let label = with_reactive_scope(document, || {
        view! { <caption content={"Display name".to_string()} /> }
    });
    let (greeting_text, set_greeting_text) = create_signal(greeting_label(""));
    let greeting = with_reactive_scope(document, || view! { <caption content={greeting_text} /> });
    let greeting_text_node = document.shadow_root(greeting);
    document.set_text_align(greeting_text_node, TextAlign::End, TextAlign::Center);
    let header = unstyled::centered_row(document, 12.0);
    document.append_child(header, label, ItemSize::Intrinsic);
    document.append_child(header, greeting, ItemSize::Percent(100.0));

    let input = styled::text_input(document, "");
    styled::set_text_input_placeholder(document, input, "Type a name");
    styled::set_text_input_on_change(document, input, move |document, value| {
        with_reactive_scope(document, || set_greeting_text.set(greeting_label(&value)));
    });

    let hint = with_reactive_scope(document, || {
        view! { <paragraph content={"Click to place the caret, drag to select, and Ctrl+Z to undo.".to_string()} /> }
    });

    let column = unstyled::column(document, 12.0);
    document.append_child(column, header, ItemSize::Intrinsic);
    document.append_child(column, input, ItemSize::Intrinsic);
    document.append_child(column, hint, ItemSize::Intrinsic);
    column
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

fn build_choice_controls(document: &mut Document) -> NodeId {
    let modes = ["Automatic", "Manual", "Scheduled"];
    let (mode_status_text, set_mode_status_text) = create_signal("Automatic updates".to_string());
    let (mode_label, mode, mode_status) = with_reactive_scope(document, || {
        (
            view! { <caption content={"Update mode".to_string()} /> },
            view! {
                <radio_group labels={vec!["Automatic".to_string(), "Manual".to_string(), "Scheduled".to_string()]} selected={Some(0)} on_change={Box::new(move |document: &mut Document, selected| {
                    if let Some(index) = selected {
                        let text = format!("{} updates", modes[index]);
                        with_reactive_scope(document, || set_mode_status_text.set(text));
                    }
                })} />
            },
            view! { <caption content={mode_status_text} /> },
        )
    });

    let colors = ["Amber", "Blue", "Green", "Purple"];
    let (color_status_text, set_color_status_text) = create_signal("Blue selected".to_string());
    let (color_label, color, color_status) = with_reactive_scope(document, || {
        (
            view! { <caption content={"Highlight color (type to search)".to_string()} /> },
            view! {
                <listbox labels={vec!["Amber".to_string(), "Blue".to_string(), "Green".to_string(), "Purple".to_string()]} selected={Some(1)} on_change={Box::new(move |document: &mut Document, selected| {
                    if let Some(index) = selected {
                        let text = format!("{} selected", colors[index]);
                        with_reactive_scope(document, || set_color_status_text.set(text));
                    }
                })} />
            },
            view! { <caption content={color_status_text} /> },
        )
    });
    let (pin_status_text, set_pin_status_text) = create_signal("Selection is unpinned".to_string());
    let (pin, pin_status) = with_reactive_scope(document, || {
        (
            view! {
                <toggle_button label={"Pin selection".to_string()} pressed={false} on_change={Box::new(move |document: &mut Document, pressed| {
                    let text = if pressed {
                        "Selection is pinned"
                    } else {
                        "Selection is unpinned"
                    }
                    .to_string();
                    with_reactive_scope(document, || set_pin_status_text.set(text));
                })} />
            },
            view! { <caption content={pin_status_text} /> },
        )
    });
    let left = unstyled::column(document, 8.0);
    for child in [mode_label, mode, mode_status, pin, pin_status] {
        document.append_child(left, child, ItemSize::Intrinsic);
    }
    let right = unstyled::column(document, 8.0);
    for child in [color_label, color, color_status] {
        document.append_child(right, child, ItemSize::Intrinsic);
    }
    let row = unstyled::row(document, 20.0);
    document.append_child(row, left, ItemSize::Percent(50.0));
    document.append_child(row, right, ItemSize::Percent(50.0));
    row
}

fn build_menu_controls(document: &mut Document) -> NodeId {
    let fruits: Vec<String> = ["Apple", "Banana", "Cherry", "Date", "Grape", "Mango"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let fruit_label = with_reactive_scope(document, || {
        view! { <caption content={"Favorite fruit (type to search)".to_string()} /> }
    });
    let fruit = styled::select(document, &fruits, Some(0));
    let (fruit_status_text, set_fruit_status_text) = create_signal("Apple selected".to_string());
    let fruit_status = with_reactive_scope(
        document,
        || view! { <caption content={fruit_status_text} /> },
    );
    let fruit_names = fruits.clone();
    styled::set_select_on_change(document, fruit, move |document, selected| {
        let text = selected
            .and_then(|index| fruit_names.get(index))
            .map_or_else(
                || "Nothing selected".to_owned(),
                |label| format!("{label} selected"),
            );
        with_reactive_scope(document, || set_fruit_status_text.set(text));
    });
    let left = unstyled::column(document, 8.0);
    document.append_child(left, fruit_label, ItemSize::Intrinsic);
    document.append_child(left, fruit, ItemSize::Intrinsic);
    document.append_child(left, fruit_status, ItemSize::Intrinsic);

    let region_label = with_reactive_scope(document, || {
        view! { <caption content={"Right-click the card below".to_string()} /> }
    });
    let region_hint = with_reactive_scope(document, || {
        view! { <paragraph content={"The Share item opens a submenu on hover or Right Arrow; Left Arrow closes it.".to_string()} /> }
    });
    let region_column = unstyled::column(document, 4.0);
    document.append_child(region_column, region_label, ItemSize::Intrinsic);
    document.append_child(region_column, region_hint, ItemSize::Intrinsic);
    let region_card = with_reactive_scope(document, || view! { <card>{region_column}</card> });

    let (menu_status_text, set_menu_status_text) = create_signal("Nothing chosen yet".to_string());
    let menu_status = with_reactive_scope(
        document,
        || view! { <caption content={menu_status_text} /> },
    );
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
    let context_menu = styled::context_menu(document, region_card, items);
    styled::set_context_menu_on_select(document, context_menu, move |document, path| {
        let label = match path.as_slice() {
            [0] => "Copy".to_owned(),
            [1] => "Paste".to_owned(),
            [2, 0] => "Share > Email".to_owned(),
            [2, 1] => "Share > Link".to_owned(),
            other => format!("{other:?}"),
        };
        with_reactive_scope(document, || {
            set_menu_status_text.set(format!("Chose: {label}"))
        });
    });
    let right = unstyled::column(document, 8.0);
    document.append_child(right, context_menu, ItemSize::Intrinsic);
    document.append_child(right, menu_status, ItemSize::Intrinsic);

    let row = unstyled::row(document, 20.0);
    document.append_child(row, left, ItemSize::Percent(50.0));
    document.append_child(row, right, ItemSize::Percent(50.0));
    row
}
