use std::cell::RefCell;

use eframe::egui;

thread_local! {
    static OPEN: RefCell<bool> = const { RefCell::new(false) };
}

/// Nothing to set up: running an arbitrary wasm module is only wired up for
/// the web build so far.
pub(crate) fn install(_creation_context: &eframe::CreationContext<'_>) {}

pub(crate) fn open() {
    OPEN.with(|open| *open.borrow_mut() = true);
}

pub(crate) fn show(ctx: &egui::Context) {
    OPEN.with(|open| {
        let mut is_open = *open.borrow();
        if !is_open {
            return;
        }
        egui::Window::new("Wasm Demo")
            .open(&mut is_open)
            .default_size([360.0, 140.0])
            .show(ctx, |ui| {
                ui.label("Wasm execution is currently supported in the web build only.");
            });
        *open.borrow_mut() = is_open;
    });
}
