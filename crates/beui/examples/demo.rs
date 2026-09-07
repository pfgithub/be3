use std::rc::Rc;

use beui::demo::{Demo, LocalCounter};
use beui::{Color32, Context, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("beui demo", DemoApp::new())
}

struct DemoApp {
    demo: Demo,
}

impl DemoApp {
    fn new() -> Self {
        Self {
            demo: Demo::new(Rc::new(LocalCounter::default())),
        }
    }
}

impl beui::App for DemoApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.demo.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        self.demo.background()
    }
}
