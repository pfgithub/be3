use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use beui::styled::theme::{
    ACCENT, ACCENT_SOFT, BACKGROUND, RADIUS, SCROLLBAR_WIDTH, SEPARATOR_HEIGHT, SURFACE,
    SURFACE_RAISED, TEXT_MUTED,
};
use beui::styled::{self, ButtonVariant};
use beui::{unstyled, Color32, Context, CursorIcon, Document, ItemSize, NodeId, Rect, TextAlign};

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

struct Rows {
    status: NodeId,
    selected: Cell<Option<usize>>,
    timings: Cell<bool>,
    visuals: RefCell<HashMap<usize, Weak<RowVisual>>>,
}

impl Rows {
    fn new(status: NodeId) -> Self {
        Self {
            status,
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
        document.set_text(self.status, format!("Row {index} selected"));
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
    let label = styled::body(document, format!("Row {index}"));
    let value = styled::caption(document, format!("{} ms", 7 + index * 3 % 91));
    document.set_text_align(value, TextAlign::End, TextAlign::Center);
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

    let catcher = document.create_click_catcher(CursorIcon::PointingHand);
    document.set_click_catcher_child(catcher, fill);

    let visual = Rc::new(RowVisual {
        index,
        fill,
        value,
        timing,
        hovered: Cell::new(false),
        rows: rows.clone(),
    });
    rows.register(&visual);
    visual.apply(document);

    let hovered = visual.clone();
    document.set_click_catcher_on_hover_change(catcher, move |document, is_hovered| {
        hovered.hovered.set(is_hovered);
        hovered.apply(document);
    });

    let clicked = visual;
    document.set_click_catcher_on_click(catcher, move |document| {
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

struct DemoApp {
    document: Document,
}

impl DemoApp {
    fn new() -> Self {
        let mut document = Document::new();

        let counter_value = styled::display(&mut document, "0");
        let counter = Rc::new(Cell::new(0i32));

        let header = build_header(&mut document, &counter, counter_value);
        let body = build_body(&mut document, counter_value);

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

fn build_header(document: &mut Document, counter: &Rc<Cell<i32>>, counter_value: NodeId) -> NodeId {
    let title = styled::title(document, "beui");
    let subtitle = styled::caption(document, "retained mode ui");
    let brand = unstyled::centered_row(document, 10.0);
    document.append_child(brand, title, ItemSize::Intrinsic);
    document.append_child(brand, subtitle, ItemSize::Intrinsic);

    let reset = styled::button(document, "Reset", ButtonVariant::Secondary);
    let decrement = styled::button(document, "-", ButtonVariant::Primary);
    let increment = styled::button(document, "+", ButtonVariant::Primary);

    let reset_counter = counter.clone();
    styled::set_button_on_click(document, reset, move |document| {
        reset_counter.set(0);
        document.set_text(counter_value, "0");
    });

    let decrement_counter = counter.clone();
    styled::set_button_on_click(document, decrement, move |document| {
        decrement_counter.set(decrement_counter.get() - 1);
        document.set_text(counter_value, decrement_counter.get().to_string());
    });

    let increment_counter = counter.clone();
    styled::set_button_on_click(document, increment, move |document| {
        increment_counter.set(increment_counter.get() + 1);
        document.set_text(counter_value, increment_counter.get().to_string());
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

fn build_body(document: &mut Document, counter_value: NodeId) -> NodeId {
    let panes = unstyled::row(document, 20.0);
    let sidebar = build_sidebar(document);
    let main = build_main(document, counter_value);
    document.append_child(panes, sidebar, ItemSize::Percent(32.0));
    document.append_child(panes, main, ItemSize::Percent(68.0));

    let padding = document.create_padding(BODY_PADDING, BODY_PADDING);
    document.set_padding_child(padding, panes);
    padding
}

fn build_sidebar(document: &mut Document) -> NodeId {
    let about = styled::paragraph(
        document,
        "beui keeps a retained tree of nodes. Base nodes carry behaviour only, unstyled \
         components compose them, and the styled components paint them.",
    );
    let about_section = styled::accordion(document, "About", about, true);

    let line = styled::separator(document);

    let tab = styled::shortcut(document, "Tab", "move focus to the next control");
    let shift_tab = styled::shortcut(document, "Shift+Tab", "move focus back");
    let enter = styled::shortcut(document, "Enter", "activate the focused control");
    let arrows = styled::shortcut(document, "Arrows", "adjust the focused slider");
    let wheel = styled::shortcut(document, "Wheel", "scroll the row list");
    let inspect = styled::shortcut(document, "Ctrl+Shift+I", "open the inspector");
    let pick = styled::shortcut(document, "Ctrl+Shift+C", "pick a node to inspect");

    let keys = unstyled::column(document, 12.0);
    document.append_child(keys, tab, ItemSize::Intrinsic);
    document.append_child(keys, shift_tab, ItemSize::Intrinsic);
    document.append_child(keys, enter, ItemSize::Intrinsic);
    document.append_child(keys, arrows, ItemSize::Intrinsic);
    document.append_child(keys, wheel, ItemSize::Intrinsic);
    document.append_child(keys, inspect, ItemSize::Intrinsic);
    document.append_child(keys, pick, ItemSize::Intrinsic);
    let keyboard_section = styled::accordion(document, "Keyboard", keys, true);

    let content = unstyled::column(document, 12.0);
    document.append_child(content, about_section, ItemSize::Intrinsic);
    document.append_child(content, line, ItemSize::Fixed(SEPARATOR_HEIGHT));
    document.append_child(content, keyboard_section, ItemSize::Intrinsic);

    styled::card(document, content)
}

fn build_main(document: &mut Document, counter_value: NodeId) -> NodeId {
    let counter_label = styled::caption(document, "Counter");
    let counter_hint = styled::paragraph(
        document,
        "Click the header buttons, or focus one with Tab and press Enter.",
    );
    let counter_column = unstyled::column(document, 4.0);
    document.append_child(counter_column, counter_label, ItemSize::Intrinsic);
    document.append_child(counter_column, counter_value, ItemSize::Intrinsic);
    document.append_child(counter_column, counter_hint, ItemSize::Intrinsic);
    let counter_card = styled::card(document, counter_column);

    let list_title = styled::heading(document, format!("Rows ({ROW_COUNT})"));
    let status = styled::caption(document, "Nothing selected");
    document.set_text_align(status, TextAlign::End, TextAlign::Center);
    let list_header = unstyled::centered_row(document, 12.0);
    document.append_child(list_header, list_title, ItemSize::Intrinsic);
    document.append_child(list_header, status, ItemSize::Percent(100.0));

    let list_line = styled::separator(document);

    let scroll = document.create_scroll();
    let rows = Rc::new(Rows::new(status));
    install_rows(document, scroll, &rows, false);
    let bar = styled::scrollbar(document, scroll);

    let area = unstyled::row(document, 10.0);
    document.append_child(area, scroll, ItemSize::Percent(100.0));
    document.append_child(area, bar, ItemSize::Fixed(SCROLLBAR_WIDTH));

    let list_column = unstyled::column(document, 12.0);
    document.append_child(list_column, list_header, ItemSize::Intrinsic);
    document.append_child(list_column, list_line, ItemSize::Fixed(SEPARATOR_HEIGHT));
    document.append_child(list_column, area, ItemSize::Percent(100.0));
    let list_card = styled::card(document, list_column);

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

    let panels = unstyled::column(document, 0.0);
    document.append_child(panels, list_visibility, ItemSize::Intrinsic);
    document.append_child(panels, load_visibility, ItemSize::Intrinsic);

    let tabs = styled::tabs(document, &["List", "Load"], 0);
    styled::add_tabs_on_change(document, tabs, move |document, selected| {
        document.set_visible(list_visibility, selected == 0);
        document.set_visible(load_visibility, selected == 1);
    });

    let column = unstyled::column(document, 16.0);
    document.append_child(column, tabs, ItemSize::Intrinsic);
    document.append_child(column, panels, ItemSize::Intrinsic);
    styled::card(document, column)
}

fn build_list_controls(document: &mut Document, scroll: NodeId, rows: &Rc<Rows>) -> NodeId {
    let timings = styled::checkbox(document, "Show timings", true);
    let timing_rows = rows.clone();
    styled::add_checkbox_on_change(document, timings, move |document, checked| {
        timing_rows.show_timings(document, checked);
    });

    let compact = styled::switch(document, false);
    let compact_label = styled::body(document, "Compact rows");
    let compact_line = unstyled::centered_row(document, 12.0);
    document.append_child(compact_line, compact, ItemSize::Intrinsic);
    document.append_child(compact_line, compact_label, ItemSize::Percent(100.0));

    let compact_rows = rows.clone();
    styled::add_switch_on_change(document, compact, move |document, on| {
        install_rows(document, scroll, &compact_rows, on);
    });

    let column = unstyled::column(document, 12.0);
    document.append_child(column, timings, ItemSize::Intrinsic);
    document.append_child(column, compact_line, ItemSize::Intrinsic);
    column
}

fn build_load_controls(document: &mut Document) -> NodeId {
    let label = styled::caption(document, "Simulated load");
    let readout = styled::caption(document, percent_label(0.4));
    document.set_text_align(readout, TextAlign::End, TextAlign::Center);
    let header = unstyled::centered_row(document, 12.0);
    document.append_child(header, label, ItemSize::Intrinsic);
    document.append_child(header, readout, ItemSize::Percent(100.0));

    let bar = styled::progress(document, 0.4);
    let slider = styled::slider(document, 0.4);
    styled::add_slider_on_change(document, slider, move |document, value| {
        styled::set_progress_value(document, bar, value);
        document.set_text(readout, percent_label(value));
    });

    let column = unstyled::column(document, 12.0);
    document.append_child(column, header, ItemSize::Intrinsic);
    document.append_child(column, slider, ItemSize::Intrinsic);
    document.append_child(column, bar, ItemSize::Intrinsic);
    column
}

fn percent_label(value: f32) -> String {
    format!("{}%", (value * 100.0).round())
}

impl beui::App for DemoApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        BACKGROUND
    }
}
