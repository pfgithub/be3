use std::rc::Rc;

use block_editor_plugin::beui::reactive::{
    create_memo, create_signal, view, with_reactive_scope, CenteredRowBuilder, ColumnBuilder,
    FillBuilder, PaddingBuilder, WriteSignal,
};
use block_editor_plugin::beui::styled::theme::BACKGROUND;
use block_editor_plugin::beui::styled::{ButtonBuilder, ButtonVariant, DisplayBuilder};
use block_editor_plugin::beui::{Color32, Context, Document, Rect};

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

        let (value, reset, decrement, increment, root) = with_reactive_scope(&mut document, || {
            let reset_counter = counter.clone();
            let reset_set_count = set_count.clone();
            let decrement_counter = counter.clone();
            let decrement_set_count = set_count.clone();
            let increment_counter = counter.clone();
            let increment_set_count = set_count.clone();

            let value = view! {
                <display content={create_memo(move || count.get().to_string())} />
            };
            let reset = view! {
                <button label={"Reset".to_string()} variant={ButtonVariant::Secondary} on_click={Box::new(move |_document| {
                    reset_counter.reset();
                    reset_set_count.set(reset_counter.value());
                })} />
            };
            let decrement = view! {
                <button label={"-".to_string()} variant={ButtonVariant::Primary} on_click={Box::new(move |_document| {
                    decrement_counter.decrement();
                    decrement_set_count.set(decrement_counter.value());
                })} />
            };
            let increment = view! {
                <button label={"+".to_string()} variant={ButtonVariant::Primary} on_click={Box::new(move |_document| {
                    increment_counter.increment();
                    increment_set_count.set(increment_counter.value());
                })} />
            };

            let root = view! {
                <fill color={BACKGROUND} radius={0}>
                    <padding horizontal={PADDING} vertical={PADDING}>
                        <column spacing={16.0}>
                            {value}
                            <centered_row spacing={10.0}>
                                @fixed(BUTTON_WIDTH) {decrement}
                                @fixed(BUTTON_WIDTH) {increment}
                                {reset}
                            </centered_row>
                        </column>
                    </padding>
                </fill>
            };

            (value, reset, decrement, increment, root)
        });

        document.set_test_id(document.shadow_root(value), "counter.value");
        document.set_test_id(reset, "counter.reset");
        document.set_test_id(decrement, "counter.decrement");
        document.set_test_id(increment, "counter.increment");
        document.set_root(root);

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
