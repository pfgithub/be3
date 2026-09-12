use beui::reactive::ItemSize;
use std::rc::Rc;

use block_editor_plugin::beui::reactive::{
    build, clone, create_memo, create_signal, view, with_reactive_scope, CenteredRow, Column, Fill,
    Padding, WriteSignal,
};
use block_editor_plugin::beui::styled::theme::BACKGROUND;
use block_editor_plugin::beui::styled::{Button, ButtonVariant, Display};
use block_editor_plugin::beui::{Color32, Context, Document, Rect};

const PADDING: f32 = 20.0;
const BUTTON_WIDTH: f32 = 44.0;

fn step(
    counter: &Rc<dyn Counter>,
    set_count: &WriteSignal<i64>,
    action: fn(&(dyn Counter + 'static)),
) -> impl FnMut() + 'static {
    let counter = counter.clone();
    let set_count = set_count.clone();
    move || {
        action(counter.as_ref());
        set_count.set(counter.value());
    }
}

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
        let (count, set_count) = create_signal(counter.value());
        let sink = set_count.clone();

        let document = build(move || {
            let reset = step(&counter, &sink, Counter::reset);
            let decrement = step(&counter, &sink, Counter::decrement);
            let increment = step(&counter, &sink, Counter::increment);

            view! {
                <Fill color=BACKGROUND radius=0>
                    <Padding horizontal=PADDING vertical=PADDING>
                        <Column spacing=16.0>
                            <Display
                                content={create_memo(clone!(count -> move || count.get().to_string()))}
                                @test_id={"counter.value"}
                            />
                            <CenteredRow spacing=10.0>
                                <Button @sizing=ItemSize::Fixed(BUTTON_WIDTH)
                                    label="-"
                                    variant=ButtonVariant::Primary
                                    @test_id={"counter.decrement"}
                                    on_click={decrement}
                                />
                                <Button @sizing=ItemSize::Fixed(BUTTON_WIDTH)
                                    label="+"
                                    variant=ButtonVariant::Primary
                                    @test_id={"counter.increment"}
                                    on_click={increment}
                                />
                                <Button
                                    label="Reset"
                                    variant=ButtonVariant::Secondary
                                    @test_id={"counter.reset"}
                                    on_click={reset}
                                />
                            </CenteredRow>
                        </Column>
                    </Padding>
                </Fill>
            }
        });

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
