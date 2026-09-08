use beui::reactive::{button, column, create_signal, fixed, intrinsic, on_click, row, text};
use beui::{Color32, Context, Document, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("beui counter", CounterApp::new())
}

struct CounterApp {
    document: Document,
}

impl CounterApp {
    fn new() -> Self {
        let mut document = Document::new();

        let (count, set_count) = create_signal(0i64);

        let decrement = {
            let set_count = set_count.clone();
            let label = text(&mut document, "-");
            button(
                &mut document,
                label,
                on_click(move || set_count.update(|count| *count -= 1)),
            )
        };
        let value = text(&mut document, count);
        let increment = {
            let label = text(&mut document, "+");
            button(
                &mut document,
                label,
                on_click(move || set_count.update(|count| *count += 1)),
            )
        };

        let controls = row(
            &mut document,
            8.0,
            [
                fixed(decrement, 32.0),
                intrinsic(value),
                fixed(increment, 32.0),
            ],
        );
        let app = column(&mut document, 0.0, [intrinsic(controls)]);
        document.set_root(app);

        Self { document }
    }
}

impl beui::App for CounterApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        Color32::BLACK
    }
}
