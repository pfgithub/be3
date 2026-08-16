mod engine;
mod renderer;

use std::cell::RefCell;

use eframe::egui;

use engine::GuestModule;
use renderer::{WasmDemoCallback, WasmDemoFrame};

const WASM_DEMO_BYTES: &[u8] = include_bytes!(env!("WASM_DEMO_PATH"));

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[derive(Default)]
struct State {
    open: bool,
    module: Option<Result<GuestModule, String>>,
}

/// Sets up the wasm-demo renderer. eframe hands out its wgpu render state
/// once, at startup, so it is claimed here rather than when the window opens.
pub(crate) fn install(creation_context: &eframe::CreationContext<'_>) {
    renderer::install(creation_context);
}

/// Opens the wasm-demo debug window, loading the wasm-demo module if it has
/// not been loaded yet.
pub(crate) fn open() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.open = true;
        if state.module.is_none() {
            state.module = Some(GuestModule::load(WASM_DEMO_BYTES));
        }
    });
}

/// Draws the wasm-demo debug window, if open.
pub(crate) fn show(ctx: &egui::Context) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.open {
            return;
        }

        let mut open = state.open;
        egui::Window::new("Wasm Demo")
            .open(&mut open)
            .default_size([420.0, 420.0])
            .show(ctx, |ui| show_contents(ui, &mut state));
        state.open = open;
    });
}

fn show_contents(ui: &mut egui::Ui, state: &mut State) {
    match state.module.as_mut() {
        Some(Ok(module)) => show_running_module(ui, module),
        Some(Err(error)) => {
            ui.colored_label(
                egui::Color32::from_rgb(220, 90, 90),
                format!("Failed to load wasm-demo: {error}"),
            );
        }
        None => {
            ui.label("wasm-demo has not been loaded.");
        }
    }
}

fn show_running_module(ui: &mut egui::Ui, module: &mut GuestModule) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
    let size = response.rect.size().max(egui::vec2(1.0, 1.0));
    let pixels_per_point = ui.ctx().pixels_per_point();
    let time_seconds = ui.input(|input| input.time) as f32;

    match module.run_frame(time_seconds) {
        Ok(draw) => {
            let frame = WasmDemoFrame {
                viewport_size_px: [
                    (size.x * pixels_per_point).round() as u32,
                    (size.y * pixels_per_point).round() as u32,
                ],
                draw,
            };
            painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
                response.rect,
                WasmDemoCallback { frame },
            ));
            ui.ctx().request_repaint();
        }
        Err(error) => {
            painter.text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("wasm-demo trapped: {error}"),
                egui::FontId::proportional(14.0),
                egui::Color32::from_rgb(220, 90, 90),
            );
        }
    }
}
