use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui::styled::theme::{
    ACCENT, ACCENT_SOFT, BACKGROUND, RADIUS, SCROLLBAR_WIDTH, SEPARATOR_HEIGHT, SURFACE,
    SURFACE_RAISED, TEXT_MUTED,
};
use beui::styled::{self, ButtonVariant};
use beui::{unstyled, Document, ItemSize, NodeId, TextAlign};
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

const HEADER_HEIGHT: f32 = 64.0;
const HEADER_PADDING: f32 = 20.0;
const BODY_PADDING: f32 = 20.0;
const ICON_BUTTON_WIDTH: f32 = 44.0;
const ROW_COUNT: usize = 200;

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
    let label = styled::body(document, format!("Row {index}"));
    let value = styled::caption(document, format!("{} ms", 7 + index * 3 % 91));
    document.set_text_align(value, TextAlign::End, TextAlign::Center);

    let line = unstyled::centered_row(document, 12.0);
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

    let clicked = visual;
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
    unstyled::set_button_on_click(document, reset, move |document| {
        reset_counter.set(0);
        document.set_text(counter_value, "0");
    });

    let decrement_counter = counter.clone();
    unstyled::set_button_on_click(document, decrement, move |document| {
        decrement_counter.set(decrement_counter.get() - 1);
        document.set_text(counter_value, decrement_counter.get().to_string());
    });

    let increment_counter = counter.clone();
    unstyled::set_button_on_click(document, increment, move |document| {
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
    let heading = styled::heading(document, "About");
    let about = styled::paragraph(
        document,
        "beui keeps a retained tree of nodes. Base nodes carry behaviour only, unstyled \
         components compose them, and the styled components paint them.",
    );

    let line = styled::separator(document);

    let keyboard = styled::heading(document, "Keyboard");
    let tab = styled::shortcut(document, "Tab", "move focus to the next button");
    let shift_tab = styled::shortcut(document, "Shift+Tab", "move focus back");
    let enter = styled::shortcut(document, "Enter", "activate the focused button");
    let wheel = styled::shortcut(document, "Wheel", "scroll the row list");

    let content = unstyled::column(document, 12.0);
    document.append_child(content, heading, ItemSize::Intrinsic);
    document.append_child(content, about, ItemSize::Intrinsic);
    document.append_child(content, line, ItemSize::Fixed(SEPARATOR_HEIGHT));
    document.append_child(content, keyboard, ItemSize::Intrinsic);
    document.append_child(content, tab, ItemSize::Intrinsic);
    document.append_child(content, shift_tab, ItemSize::Intrinsic);
    document.append_child(content, enter, ItemSize::Intrinsic);
    document.append_child(content, wheel, ItemSize::Intrinsic);

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

    let list_title = styled::heading(document, "Rows");
    let status = styled::caption(document, "Nothing selected");
    document.set_text_align(status, TextAlign::End, TextAlign::Center);
    let list_header = unstyled::centered_row(document, 12.0);
    document.append_child(list_header, list_title, ItemSize::Intrinsic);
    document.append_child(list_header, status, ItemSize::Percent(100.0));

    let list_line = styled::separator(document);

    let scroll = document.create_scroll();
    let selection: Selection = Rc::new(RefCell::new(None));
    for index in 0..ROW_COUNT {
        let row = scroll_row(document, index, &selection, status);
        document.append_scroll_item(scroll, row);
    }
    let bar = styled::scrollbar(document, scroll);

    let area = unstyled::row(document, 10.0);
    document.append_child(area, scroll, ItemSize::Percent(100.0));
    document.append_child(area, bar, ItemSize::Fixed(SCROLLBAR_WIDTH));

    let list_column = unstyled::column(document, 12.0);
    document.append_child(list_column, list_header, ItemSize::Intrinsic);
    document.append_child(list_column, list_line, ItemSize::Fixed(SEPARATOR_HEIGHT));
    document.append_child(list_column, area, ItemSize::Percent(100.0));
    let list_card = styled::card(document, list_column);

    let main = unstyled::column(document, 20.0);
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
