use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui::{Align, Direction, Document, ItemSize, NodeId, ScrollPosition, TextAlign};
use eframe::egui;
use egui::{Color32, CursorIcon};

fn main() -> eframe::Result {
    eframe::run_native(
        "beui demo",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            ..Default::default()
        },
        Box::new(|_cc| Ok(Box::new(DemoApp::new()))),
    )
}

const BACKGROUND: Color32 = Color32::from_rgb(14, 17, 23);
const SURFACE: Color32 = Color32::from_rgb(22, 26, 34);
const SURFACE_RAISED: Color32 = Color32::from_rgb(31, 37, 48);
const BORDER: Color32 = Color32::from_rgb(42, 50, 64);
const TEXT: Color32 = Color32::from_rgb(230, 235, 243);
const TEXT_MUTED: Color32 = Color32::from_rgb(141, 153, 174);
const ACCENT: Color32 = Color32::from_rgb(82, 137, 255);
const ACCENT_HOVER: Color32 = Color32::from_rgb(110, 159, 255);
const ACCENT_ACTIVE: Color32 = Color32::from_rgb(58, 106, 212);
const ACCENT_SOFT: Color32 = Color32::from_rgb(33, 48, 84);
const ON_ACCENT: Color32 = Color32::from_rgb(247, 250, 255);
const SCROLL_THUMB: Color32 = Color32::from_rgb(60, 71, 92);

const FONT_SMALL: f32 = 12.0;
const FONT_BODY: f32 = 14.0;
const FONT_HEADING: f32 = 16.0;
const FONT_TITLE: f32 = 21.0;
const FONT_DISPLAY: f32 = 46.0;

const RADIUS: u8 = 6;
const CARD_RADIUS: u8 = 10;
const HEADER_HEIGHT: f32 = 64.0;
const SCROLLBAR_WIDTH: f32 = 6.0;
const ICON_BUTTON_WIDTH: f32 = 44.0;

fn text(document: &mut Document, content: impl Into<String>, size: f32, color: Color32) -> NodeId {
    let id = document.create_text(content, size, color);
    document.set_text_align(id, TextAlign::Start, TextAlign::Center);
    id
}

fn paragraph(document: &mut Document, content: impl Into<String>) -> NodeId {
    let id = document.create_text(content, FONT_BODY, TEXT_MUTED);
    document.set_text_wrap(id, true);
    id
}

fn column(document: &mut Document, spacing: f32) -> NodeId {
    document.create_list(Direction::Vertical, spacing)
}

fn row(document: &mut Document, spacing: f32) -> NodeId {
    document.create_list(Direction::Horizontal, spacing)
}

fn centered_row(document: &mut Document, spacing: f32) -> NodeId {
    let id = row(document, spacing);
    document.set_list_align(id, Align::Center);
    id
}

fn spacer(document: &mut Document) -> NodeId {
    document.create_fill(Color32::TRANSPARENT, 0)
}

fn separator(document: &mut Document) -> NodeId {
    document.create_fill(BORDER, 0)
}

fn bordered(document: &mut Document, child: NodeId, corner_radius: u8) -> NodeId {
    let outline = document.create_outline(BORDER, 1.0, corner_radius, 0.0);
    document.set_outline_visible(outline, true);
    document.set_outline_child(outline, child);
    outline
}

fn card(document: &mut Document, child: NodeId) -> NodeId {
    let padding = document.create_padding(18.0, 16.0);
    document.set_padding_child(padding, child);
    let fill = document.create_fill(SURFACE, CARD_RADIUS);
    document.set_fill_child(fill, padding);
    bordered(document, fill, CARD_RADIUS)
}

fn chip(document: &mut Document, label: &str) -> NodeId {
    let label = text(document, label, FONT_SMALL, TEXT);
    document.set_text_align(label, TextAlign::Center, TextAlign::Center);
    let padding = document.create_padding(8.0, 3.0);
    document.set_padding_child(padding, label);
    let fill = document.create_fill(SURFACE_RAISED, 4);
    document.set_fill_child(fill, padding);
    bordered(document, fill, 4)
}

fn shortcut(document: &mut Document, keys: &str, description: &str) -> NodeId {
    let keys = chip(document, keys);
    let description = text(document, description, FONT_SMALL, TEXT_MUTED);
    document.set_text_wrap(description, true);
    let line = centered_row(document, 10.0);
    document.append_child(line, keys, ItemSize::Intrinsic);
    document.append_child(line, description, ItemSize::Percent(100.0));
    line
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ButtonStyle {
    Primary,
    Secondary,
}

impl ButtonStyle {
    fn fill(self, hovered: bool, active: bool) -> Color32 {
        match (self, hovered, active) {
            (ButtonStyle::Primary, _, true) => ACCENT_ACTIVE,
            (ButtonStyle::Primary, true, false) => ACCENT_HOVER,
            (ButtonStyle::Primary, false, false) => ACCENT,
            (ButtonStyle::Secondary, _, true) => BORDER,
            (ButtonStyle::Secondary, true, false) => SURFACE_RAISED,
            (ButtonStyle::Secondary, false, false) => SURFACE,
        }
    }

    fn label(self) -> Color32 {
        match self {
            ButtonStyle::Primary => ON_ACCENT,
            ButtonStyle::Secondary => TEXT,
        }
    }
}

fn button(document: &mut Document, label: &str, style: ButtonStyle) -> NodeId {
    let host = document.create_button();

    let label = document.create_text(label, FONT_BODY, style.label());
    document.set_text_align(label, TextAlign::Center, TextAlign::Center);

    let padding = document.create_padding(16.0, 9.0);
    document.set_padding_child(padding, label);

    let fill = document.create_fill(style.fill(false, false), RADIUS);
    document.set_fill_child(fill, padding);

    let border = document.create_outline(BORDER, 1.0, RADIUS, 0.0);
    document.set_outline_visible(border, style == ButtonStyle::Secondary);
    document.set_outline_child(border, fill);

    let ring = document.create_outline(ACCENT, 2.0, RADIUS + 4, 6.0);
    document.set_outline_child(ring, border);

    document.set_button_child(host, ring);

    let state = Rc::new(Cell::new((false, false)));

    let hover_state = state.clone();
    document.set_button_on_hover_change(host, move |document, hovered| {
        let (_, active) = hover_state.get();
        hover_state.set((hovered, active));
        document.set_fill_color(fill, style.fill(hovered, active));
    });

    let active_state = state.clone();
    document.set_button_on_active_change(host, move |document, active| {
        let (hovered, _) = active_state.get();
        active_state.set((hovered, active));
        document.set_fill_color(fill, style.fill(hovered, active));
    });

    document.set_button_on_focus_change(host, move |document, focused| {
        document.set_outline_visible(ring, focused);
    });

    host
}

struct RowVisual {
    fill: NodeId,
    value: NodeId,
    hovered: Cell<bool>,
    selected: Cell<bool>,
}

impl RowVisual {
    fn apply(&self, document: &mut Document) {
        let fill = match (self.selected.get(), self.hovered.get()) {
            (true, _) => ACCENT_SOFT,
            (false, true) => SURFACE_RAISED,
            (false, false) => Color32::TRANSPARENT,
        };
        document.set_fill_color(self.fill, fill);
        let value = if self.selected.get() {
            ACCENT
        } else {
            TEXT_MUTED
        };
        document.set_text_color(self.value, value);
    }
}

type Selection = Rc<RefCell<Option<Rc<RowVisual>>>>;

fn scroll_row(
    document: &mut Document,
    index: usize,
    selection: &Selection,
    status: NodeId,
) -> NodeId {
    let label = text(document, format!("Row {index}"), FONT_BODY, TEXT);
    let value = text(
        document,
        format!("{} ms", 7 + index * 3 % 91),
        FONT_SMALL,
        TEXT_MUTED,
    );
    document.set_text_align(value, TextAlign::End, TextAlign::Center);

    let line = centered_row(document, 12.0);
    document.append_child(line, label, ItemSize::Percent(100.0));
    document.append_child(line, value, ItemSize::Intrinsic);

    let padding = document.create_padding(12.0, 9.0);
    document.set_padding_child(padding, line);

    let fill = document.create_fill(Color32::TRANSPARENT, RADIUS);
    document.set_fill_child(fill, padding);

    let catcher = document.create_click_catcher(CursorIcon::PointingHand);
    document.set_click_catcher_child(catcher, fill);

    let visual = Rc::new(RowVisual {
        fill,
        value,
        hovered: Cell::new(false),
        selected: Cell::new(false),
    });

    let hovered = visual.clone();
    document.set_click_catcher_on_hover_change(catcher, move |document, is_hovered| {
        hovered.hovered.set(is_hovered);
        hovered.apply(document);
    });

    let clicked = visual.clone();
    let selection = selection.clone();
    document.set_click_catcher_on_click(catcher, move |document| {
        let previous = selection.borrow_mut().take();
        if let Some(previous) = previous {
            previous.selected.set(false);
            previous.apply(document);
        }
        clicked.selected.set(true);
        clicked.apply(document);
        *selection.borrow_mut() = Some(clicked.clone());
        document.set_text(status, format!("Row {index} selected"));
    });

    catcher
}

fn scrollbar(document: &mut Document, scroll: NodeId) -> NodeId {
    let before = spacer(document);
    let thumb = document.create_fill(Color32::TRANSPARENT, 3);
    let after = spacer(document);

    let track = column(document, 0.0);
    document.append_child(track, before, ItemSize::Percent(0.0));
    document.append_child(track, thumb, ItemSize::Percent(100.0));
    document.append_child(track, after, ItemSize::Percent(0.0));

    let background = document.create_fill(SURFACE_RAISED, 3);
    document.set_fill_child(background, track);

    document.set_scroll_on_change(scroll, move |document, position: ScrollPosition| {
        let scrollable = position.max_offset() > 0.0;
        let visible = if position.content > 0.0 {
            (position.viewport / position.content).clamp(0.08, 1.0)
        } else {
            1.0
        };
        let progress = if scrollable {
            position.offset / position.max_offset()
        } else {
            0.0
        };
        let rest = 100.0 - visible * 100.0;
        document.set_child_size(track, before, ItemSize::Percent(rest * progress));
        document.set_child_size(track, thumb, ItemSize::Percent(visible * 100.0));
        document.set_child_size(track, after, ItemSize::Percent(rest * (1.0 - progress)));
        let color = if scrollable {
            SCROLL_THUMB
        } else {
            Color32::TRANSPARENT
        };
        document.set_fill_color(thumb, color);
    });

    background
}

struct DemoApp {
    document: Document,
}

impl DemoApp {
    fn new() -> Self {
        let mut document = Document::new();

        let counter_value = text(&mut document, "0", FONT_DISPLAY, TEXT);
        let counter = Rc::new(Cell::new(0i32));

        let header = build_header(&mut document, &counter, counter_value);
        let body = build_body(&mut document, counter_value);

        let root_column = column(&mut document, 0.0);
        document.append_child(root_column, header, ItemSize::Fixed(HEADER_HEIGHT));
        let header_line = separator(&mut document);
        document.append_child(root_column, header_line, ItemSize::Fixed(1.0));
        document.append_child(root_column, body, ItemSize::Percent(100.0));

        let background = document.create_fill(BACKGROUND, 0);
        document.set_fill_child(background, root_column);
        document.set_root(background);

        Self { document }
    }
}

fn build_header(document: &mut Document, counter: &Rc<Cell<i32>>, counter_value: NodeId) -> NodeId {
    let title = text(document, "beui", FONT_TITLE, TEXT);
    let subtitle = text(document, "retained mode ui", FONT_SMALL, TEXT_MUTED);
    let brand = centered_row(document, 10.0);
    document.append_child(brand, title, ItemSize::Intrinsic);
    document.append_child(brand, subtitle, ItemSize::Intrinsic);

    let reset = button(document, "Reset", ButtonStyle::Secondary);
    let decrement = button(document, "-", ButtonStyle::Primary);
    let increment = button(document, "+", ButtonStyle::Primary);

    let reset_counter = counter.clone();
    document.set_button_on_click(reset, move |document| {
        reset_counter.set(0);
        document.set_text(counter_value, "0");
    });

    let decrement_counter = counter.clone();
    document.set_button_on_click(decrement, move |document| {
        decrement_counter.set(decrement_counter.get() - 1);
        document.set_text(counter_value, decrement_counter.get().to_string());
    });

    let increment_counter = counter.clone();
    document.set_button_on_click(increment, move |document| {
        increment_counter.set(increment_counter.get() + 1);
        document.set_text(counter_value, increment_counter.get().to_string());
    });

    let gap = spacer(document);
    let bar = centered_row(document, 10.0);
    document.append_child(bar, brand, ItemSize::Intrinsic);
    document.append_child(bar, gap, ItemSize::Percent(100.0));
    document.append_child(bar, reset, ItemSize::Intrinsic);
    document.append_child(bar, decrement, ItemSize::Fixed(ICON_BUTTON_WIDTH));
    document.append_child(bar, increment, ItemSize::Fixed(ICON_BUTTON_WIDTH));

    let padding = document.create_padding(20.0, 0.0);
    document.set_padding_child(padding, bar);
    let fill = document.create_fill(SURFACE, 0);
    document.set_fill_child(fill, padding);
    fill
}

fn build_body(document: &mut Document, counter_value: NodeId) -> NodeId {
    let panes = row(document, 20.0);
    let sidebar = build_sidebar(document);
    let main = build_main(document, counter_value);
    document.append_child(panes, sidebar, ItemSize::Percent(32.0));
    document.append_child(panes, main, ItemSize::Percent(68.0));

    let padding = document.create_padding(20.0, 20.0);
    document.set_padding_child(padding, panes);
    padding
}

fn build_sidebar(document: &mut Document) -> NodeId {
    let heading = text(document, "About", FONT_HEADING, TEXT);
    let about = paragraph(
        document,
        "beui keeps a retained tree of nodes and only ships unstyled behaviour. \
         Every button, card, row and scrollbar here is styled by the demo.",
    );

    let line = separator(document);

    let keyboard = text(document, "Keyboard", FONT_HEADING, TEXT);
    let tab = shortcut(document, "Tab", "move focus to the next button");
    let shift_tab = shortcut(document, "Shift+Tab", "move focus back");
    let enter = shortcut(document, "Enter", "activate the focused button");
    let wheel = shortcut(document, "Wheel", "scroll the row list");

    let content = column(document, 12.0);
    document.append_child(content, heading, ItemSize::Intrinsic);
    document.append_child(content, about, ItemSize::Intrinsic);
    document.append_child(content, line, ItemSize::Fixed(1.0));
    document.append_child(content, keyboard, ItemSize::Intrinsic);
    document.append_child(content, tab, ItemSize::Intrinsic);
    document.append_child(content, shift_tab, ItemSize::Intrinsic);
    document.append_child(content, enter, ItemSize::Intrinsic);
    document.append_child(content, wheel, ItemSize::Intrinsic);

    card(document, content)
}

fn build_main(document: &mut Document, counter_value: NodeId) -> NodeId {
    let counter_label = text(document, "Counter", FONT_SMALL, TEXT_MUTED);
    let counter_hint = paragraph(
        document,
        "Click the header buttons, or focus one with Tab and press Enter.",
    );
    let counter_column = column(document, 4.0);
    document.append_child(counter_column, counter_label, ItemSize::Intrinsic);
    document.append_child(counter_column, counter_value, ItemSize::Intrinsic);
    document.append_child(counter_column, counter_hint, ItemSize::Intrinsic);
    let counter_card = card(document, counter_column);

    let list_title = text(document, "Rows", FONT_HEADING, TEXT);
    let status = text(document, "Nothing selected", FONT_SMALL, TEXT_MUTED);
    document.set_text_align(status, TextAlign::End, TextAlign::Center);
    let list_header = centered_row(document, 12.0);
    document.append_child(list_header, list_title, ItemSize::Intrinsic);
    document.append_child(list_header, status, ItemSize::Percent(100.0));

    let list_line = separator(document);

    let scroll = document.create_scroll();
    let selection: Selection = Rc::new(RefCell::new(None));
    for index in 0..200 {
        let row = scroll_row(document, index, &selection, status);
        document.append_scroll_item(scroll, row);
    }
    let bar = scrollbar(document, scroll);

    let area = row(document, 10.0);
    document.append_child(area, scroll, ItemSize::Percent(100.0));
    document.append_child(area, bar, ItemSize::Fixed(SCROLLBAR_WIDTH));

    let list_column = column(document, 12.0);
    document.append_child(list_column, list_header, ItemSize::Intrinsic);
    document.append_child(list_column, list_line, ItemSize::Fixed(1.0));
    document.append_child(list_column, area, ItemSize::Percent(100.0));
    let list_card = card(document, list_column);

    let main = column(document, 20.0);
    document.append_child(main, counter_card, ItemSize::Intrinsic);
    document.append_child(main, list_card, ItemSize::Percent(100.0));
    main
}

impl eframe::App for DemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx();
        self.document.show(ctx, ctx.content_rect());
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        BACKGROUND.to_normalized_gamma_f32()
    }
}
