use std::rc::Rc;

use block_editor_plugin::beui::styled::theme::BACKGROUND;
use block_editor_plugin::beui::styled::{self, ButtonVariant};
use block_editor_plugin::beui::{unstyled, Color32, Context, Document, ItemSize, NodeId, Rect};

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
    value: NodeId,
    buttons: Buttons,
}

#[derive(Clone, Copy)]
pub struct Buttons {
    pub reset: NodeId,
    pub decrement: NodeId,
    pub increment: NodeId,
}

impl CounterUi {
    pub fn new(counter: Rc<dyn Counter>) -> Self {
        let mut document = Document::new();
        let value = styled::display(&mut document, counter.value().to_string());

        let reset = styled::button(&mut document, "Reset", ButtonVariant::Secondary);
        let decrement = styled::button(&mut document, "-", ButtonVariant::Primary);
        let increment = styled::button(&mut document, "+", ButtonVariant::Primary);

        let reset_counter = counter.clone();
        styled::set_button_on_click(&mut document, reset, move |document| {
            reset_counter.reset();
            document.set_text(value, reset_counter.value().to_string());
        });

        let decrement_counter = counter.clone();
        styled::set_button_on_click(&mut document, decrement, move |document| {
            decrement_counter.decrement();
            document.set_text(value, decrement_counter.value().to_string());
        });

        let increment_counter = counter.clone();
        styled::set_button_on_click(&mut document, increment, move |document| {
            increment_counter.increment();
            document.set_text(value, increment_counter.value().to_string());
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
            value,
            buttons: Buttons {
                reset,
                decrement,
                increment,
            },
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn value_node(&self) -> NodeId {
        self.value
    }

    pub fn buttons(&self) -> Buttons {
        self.buttons
    }

    pub fn background(&self) -> Color32 {
        BACKGROUND
    }

    pub fn set_value(&mut self, value: i64) {
        self.document.set_text(self.value, value.to_string());
    }

    pub fn show(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}
