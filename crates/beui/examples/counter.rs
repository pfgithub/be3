use beui::reactive::{
    build, button, column, component, create_memo, create_signal, for_each, intrinsic, row, show,
    text, view,
};
use beui::{App, ClickHandler, Color32, Context, Document, NodeId, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("beui counter", CounterApp::new())
}

#[component]
fn history_entry(value: i64) -> NodeId {
    view! { text { string: value.to_string() } }
}

#[component]
fn app() -> NodeId {
    let (count, set_count) = create_signal(0i64);
    let (history, set_history) = create_signal(Vec::<(u64, i64)>::new());
    let (next_id, set_next_id) = create_signal(0u64);

    let is_zero = {
        let count = count.clone();
        create_memo(move || count.get() <= 0)
    };
    let is_nonzero = {
        let count = count.clone();
        create_memo(move || count.get() != 0)
    };

    let decrement_click: ClickHandler = Box::new({
        let set_count = set_count.clone();
        let set_history = set_history.clone();
        let count = count.clone();
        let next_id = next_id.clone();
        let set_next_id = set_next_id.clone();
        move |_document| {
            set_count.update(|value| *value -= 1);
            let id = next_id.get();
            set_next_id.set(id + 1);
            set_history.update(|entries| entries.push((id, count.get())));
        }
    });

    let increment_click: ClickHandler = Box::new({
        let set_count = set_count.clone();
        let set_history = set_history.clone();
        let count = count.clone();
        let next_id = next_id.clone();
        let set_next_id = set_next_id.clone();
        move |_document| {
            set_count.update(|value| *value += 1);
            let id = next_id.get();
            set_next_id.set(id + 1);
            set_history.update(|entries| entries.push((id, count.get())));
        }
    });

    let reset_click: ClickHandler = Box::new(move |_document| {
        set_count.set(0);
        set_history.set(Vec::new());
    });

    let count_text = create_memo(move || count.get().to_string());

    view! {
        column {
            spacing: 8.0,
        } [
            row {
                spacing: 8.0,
            } [
                button {
                    disabled: is_zero,
                    on_click: decrement_click,
                } [ text { string: "-".to_string() } ],
                text { string: count_text },
                button {
                    on_click: increment_click,
                } [ text { string: "+".to_string() } ],
                show {
                    condition: is_nonzero,
                    then: Box::new(move || view! {
                        button {
                            on_click: reset_click,
                        } [ text { string: "reset".to_string() } ]
                    }),
                }
            ],
            for_each {
                spacing: 4.0,
                items: history,
                key: Box::new(|(id, _)| *id),
                view: Box::new(|(_, value)| intrinsic(history_entry().value(*value).build())),
            }
        ]
    }
}

struct CounterApp {
    document: Document,
}

impl CounterApp {
    fn new() -> Self {
        Self {
            document: build(|| app().build()),
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
