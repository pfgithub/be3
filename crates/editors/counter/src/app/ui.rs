use std::rc::Rc;

use block_editor_plugin::beui::reactive::{
    create_memo, create_signal, view, with_reactive_scope, WriteSignal,
};
use block_editor_plugin::beui::styled::reactive::{ButtonBuilder, DisplayBuilder};
use block_editor_plugin::beui::styled::theme::BACKGROUND;
use block_editor_plugin::beui::styled::{self, ButtonVariant};
use block_editor_plugin::beui::{unstyled, Color32, Context, Document, ItemSize, Rect};

const PADDING: f32 = 20.0;
const BUTTON_WIDTH: f32 = 44.0;

pub trait Counter {
    fn value(&self) -> i64;
    fn increment(&self);
    fn decrement(&self);
    fn reset(&self);
}

pub struct CounterUi {
    document: Document,
    set_count: WriteSignal<i64>,
}

impl CounterUi {
    pub fn new(counter: Rc<dyn Counter>) -> Self {
        let mut document = Document::new();
        let (count, set_count) = create_signal(counter.value());

        let (value, reset, decrement, increment) = with_reactive_scope(&mut document, || {
            let value = view! {
                <display content={create_memo(move || count.get().to_string())} />
            };
            let reset = view! {
                <button label={"Reset".to_string()} variant={ButtonVariant::Secondary} />
            };
            let decrement = view! {
                <button label={"-".to_string()} variant={ButtonVariant::Primary} />
            };
            let increment = view! {
                <button label={"+".to_string()} variant={ButtonVariant::Primary} />
            };
            (value, reset, decrement, increment)
        });

        document.set_test_id(document.shadow_root(value), "counter.value");
        document.set_test_id(reset, "counter.reset");
        document.set_test_id(decrement, "counter.decrement");
        document.set_test_id(increment, "counter.increment");

        let reset_counter = counter.clone();
        let reset_set_count = set_count.clone();
        styled::set_button_on_click(&mut document, reset, move |_document| {
            reset_counter.reset();
            reset_set_count.set(reset_counter.value());
        });

        let decrement_counter = counter.clone();
        let decrement_set_count = set_count.clone();
        styled::set_button_on_click(&mut document, decrement, move |_document| {
            decrement_counter.decrement();
            decrement_set_count.set(decrement_counter.value());
        });

        let increment_counter = counter.clone();
        let increment_set_count = set_count.clone();
        styled::set_button_on_click(&mut document, increment, move |_document| {
            increment_counter.increment();
            increment_set_count.set(increment_counter.value());
        });

        let buttons_row = unstyled::centered_row(&mut document, 10.0);
        document.append_child(buttons_row, decrement, ItemSize::Fixed(BUTTON_WIDTH));
        document.append_child(buttons_row, increment, ItemSize::Fixed(BUTTON_WIDTH));
        document.append_child(buttons_row, reset, ItemSize::Intrinsic);

        let column = unstyled::column(&mut document, 16.0);
        document.append_child(column, value, ItemSize::Intrinsic);
        document.append_child(column, buttons_row, ItemSize::Intrinsic);

        let padding = document.create_padding(PADDING, PADDING);
        document.set_padding_child(padding, column);
        let background = document.create_fill(BACKGROUND, 0);
        document.set_fill_child(background, padding);
        document.set_root(background);

        Self {
            document,
            set_count,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn background(&self) -> Color32 {
        BACKGROUND
    }

    pub fn set_value(&mut self, value: i64) {
        let set_count = self.set_count.clone();
        with_reactive_scope(&mut self.document, move || set_count.set(value));
    }

    pub fn show(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}
