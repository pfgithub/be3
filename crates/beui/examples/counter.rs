use beui::reactive::{build, button, column, create_signal, intrinsic, on_click, row, text};
use beui::{App, Color32, Context, Document, NodeId, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("beui counter", CounterApp::new())
}

fn app() -> NodeId {
    let (count, set_count) = create_signal(0i64);
    let set_count_decrement = set_count.clone();

    column(
        0.0,
        [intrinsic(row(
            8.0,
            [
                intrinsic(button(
                    text("-"),
                    on_click(move || set_count_decrement.update(|count| *count -= 1)),
                )),
                intrinsic(text(count)),
                intrinsic(button(
                    text("+"),
                    on_click(move || set_count.update(|count| *count += 1)),
                )),
            ],
        ))],
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
