use beui::reactive::{
    build, button, column, component, create_memo, create_signal, for_each, intrinsic, row, show,
    text,
};
use beui::{App, Color32, Context, Document, NodeId, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("beui counter", CounterApp::new())
}

#[component]
fn history_entry(value: i64) -> NodeId {
    text(value.to_string())
}

#[component]
fn app() -> NodeId {
    let (count, set_count) = create_signal(0i64);
    let (history, set_history) = create_signal(Vec::<(u64, i64)>::new());
    let next_id = std::rc::Rc::new(std::cell::Cell::new(0u64));

    let is_zero = {
        let count = count.clone();
        create_memo(move || count.get() <= 0)
    };
    let is_nonzero = {
        let count = count.clone();
        create_memo(move || count.get() != 0)
    };

    let decrement = {
        let set_count = set_count.clone();
        let set_history = set_history.clone();
        let count = count.clone();
        let next_id = next_id.clone();
        button()
            .label(text("-"))
            .disabled(is_zero)
            .on_click(move || {
                set_count.update(|value| *value -= 1);
                let id = next_id.get();
                next_id.set(id + 1);
                set_history.update(|entries| entries.push((id, count.get())));
            })
            .build()
    };

    let increment = {
        let set_count = set_count.clone();
        let set_history = set_history.clone();
        let count = count.clone();
        let next_id = next_id.clone();
        button()
            .label(text("+"))
            .on_click(move || {
                set_count.update(|value| *value += 1);
                let id = next_id.get();
                next_id.set(id + 1);
                set_history.update(|entries| entries.push((id, count.get())));
            })
            .build()
    };

    let reset = show(is_nonzero, move || {
        let set_count = set_count.clone();
        let set_history = set_history.clone();
        button()
            .label(text("reset"))
            .on_click(move || {
                set_count.set(0);
                set_history.set(Vec::new());
            })
            .build()
    });

    let log = column(4.0, []);
    for_each(
        log,
        history,
        |(id, _)| *id,
        |(_, value)| intrinsic(history_entry(*value)),
    );

    column(
        8.0,
        [
            intrinsic(row(
                8.0,
                [
                    intrinsic(decrement),
                    intrinsic(text(count)),
                    intrinsic(increment),
                    intrinsic(reset),
                ],
            )),
            intrinsic(log),
        ],
    )
}

struct CounterApp {
    document: Document,
}

impl CounterApp {
    fn new() -> Self {
        Self {
            document: build(app),
        }
    }
}

impl App for CounterApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        Color32::BLACK
    }
}
