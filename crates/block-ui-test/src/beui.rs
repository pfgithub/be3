use beui::{Color32, Context, Event, Key, Modifiers, PointerButton, Pos2, Rect, Vec2};
use block_editor_plugin::BeuiApp;

use crate::snapshot;

mod render;

const SIZE: Vec2 = Vec2::new(800.0, 600.0);

pub struct BeuiTest<A: BeuiApp> {
    app: A,
    context: Context,
    size: Vec2,
    pixels_per_point: f32,
    events: Vec<Event>,
    modifiers: Modifiers,
    output: Option<beui::FrameOutput>,
}

impl<A: BeuiApp> BeuiTest<A> {
    pub fn new(app: A) -> Self {
        let context = Context::with_fonts(&block_editor_plugin::beui_fonts());
        context.set_pixels_per_point(1.0);
        let mut editor = Self {
            app,
            context,
            size: SIZE,
            pixels_per_point: 1.0,
            events: Vec::new(),
            modifiers: Modifiers::NONE,
            output: None,
        };
        editor.run();
        editor
    }

    pub fn app(&mut self) -> &mut A {
        &mut self.app
    }

    pub fn rect(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, self.size)
    }

    pub fn run(&mut self) {
        let events = std::mem::take(&mut self.events);
        if events.is_empty() {
            return self.step(Vec::new());
        }
        for event in events {
            self.step(vec![event]);
        }
    }

    pub fn step(&mut self, events: Vec<Event>) {
        let rect = self.rect();
        let app = &mut self.app;
        let output = self.context.run(beui::RawInput { events }, |context| {
            app.frame(context, rect)
        });
        self.output = Some(output);
    }

    pub fn hover_at(&mut self, pos: Pos2) {
        self.events.push(Event::PointerMoved(pos));
    }

    pub fn rect_of(&self, test_id: &str) -> Rect {
        self.output
            .as_ref()
            .expect("the editor has not drawn a frame yet")
            .test_id_rect(test_id)
            .unwrap_or_else(|| panic!("no element with test id {test_id:?}"))
    }

    pub fn hover(&mut self, test_id: &str) {
        self.hover_at(self.rect_of(test_id).center());
    }

    pub fn click(&mut self, test_id: &str) {
        self.click_at(self.rect_of(test_id).center());
    }

    pub fn click_at(&mut self, pos: Pos2) {
        self.hover_at(pos);
        self.events.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: self.modifiers,
        });
        self.events.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: self.modifiers,
        });
    }

    pub fn key_press(&mut self, key: Key) {
        self.key_press_modifiers(Modifiers::NONE, key);
    }

    pub fn key_press_modifiers(&mut self, modifiers: Modifiers, key: Key) {
        self.events.push(Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers,
        });
        self.events.push(Event::Key {
            key,
            pressed: false,
            repeat: false,
            modifiers,
        });
    }

    pub fn text(&mut self, text: impl Into<String>) {
        self.events.push(Event::Text(text.into()));
    }

    pub fn snapshot(&mut self, name: &str) {
        let output = self
            .output
            .as_ref()
            .expect("the editor has not drawn a frame yet");
        let painting = render::capture(output, self.size, self.pixels_per_point, Color32::BLACK)
            .expect("the painting could not be rendered");
        snapshot::assert_snapshot(name, &painting);
    }
}
