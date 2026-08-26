use std::cell::RefCell;

use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Window {
    Inspection,
    Memory,
    Textures,
    Style,
}

impl Window {
    fn title(self) -> &'static str {
        match self {
            Self::Inspection => "egui Inspection",
            Self::Memory => "egui Memory",
            Self::Textures => "egui Textures",
            Self::Style => "egui Style",
        }
    }

    fn ui(self, ctx: &egui::Context, ui: &mut egui::Ui) {
        match self {
            Self::Inspection => ctx.inspection_ui(ui),
            Self::Memory => ctx.memory_ui(ui),
            Self::Textures => ctx.texture_ui(ui),
            Self::Style => ctx.settings_ui(ui),
        }
    }
}

thread_local! {
    static OPEN: RefCell<Vec<Window>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn open(window: Window) {
    OPEN.with(|open| {
        let mut open = open.borrow_mut();
        if !open.contains(&window) {
            open.push(window);
        }
    });
}

pub(crate) fn show(ctx: &egui::Context) {
    let windows = OPEN.with(|open| open.borrow().clone());
    let mut still_open = Vec::with_capacity(windows.len());
    for window in windows {
        let mut open = true;
        egui::Window::new(window.title())
            .open(&mut open)
            .default_size([520.0, 420.0])
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| window.ui(ctx, ui));
        if open {
            still_open.push(window);
        }
    }
    OPEN.with(|open| *open.borrow_mut() = still_open);
}

pub(crate) fn debug_on_hover_toggle(ui: &mut egui::Ui) {
    let mut debug_on_hover = ui.ctx().debug_on_hover();
    if ui
        .checkbox(&mut debug_on_hover, "Debug on hover")
        .on_hover_text("Show widget rectangles and ids under the pointer")
        .changed()
    {
        ui.ctx().set_debug_on_hover(debug_on_hover);
    }
}
