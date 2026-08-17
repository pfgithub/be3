use std::cell::RefCell;

use eframe::egui;
use wasm_bindgen::JsCast;

mod adapter;
pub(super) mod renderer;

use super::input::InputAdapter;
use super::presenter::{
    PresenterCallback, PresenterCommand, PresenterState, PresenterStatus, WebFrame,
};
use adapter::WebProtocolAdapter;
const CANVAS_ID: &str = "plugin-demo-canvas";

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[derive(Default)]
struct State {
    open: bool,
    starting: bool,
    adapter: Option<WebProtocolAdapter>,
    render_available: bool,
    error: Option<String>,
    canvas_size: [u32; 2],
    input: InputAdapter,
    presenter_status: Option<PresenterStatus>,
}

pub(crate) fn install(creation_context: &eframe::CreationContext<'_>) {
    let presenter_status = renderer::install(creation_context);
    let render_available = presenter_status.is_some();
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.render_available = render_available;
        state.presenter_status = presenter_status;
        if !render_available {
            state.error = Some("wgpu is not available in this build.".to_owned());
        }
    });
}

pub(crate) fn open() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.open = true;
        if state.starting || state.adapter.is_some() || !state.render_available {
            return;
        }
        state.starting = true;
        create_canvas();
        wasm_bindgen_futures::spawn_local(async {
            let result = WebProtocolAdapter::start(CANVAS_ID).await;
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                state.starting = false;
                match result {
                    Ok(adapter) if state.open => state.adapter = Some(adapter),
                    Ok(mut adapter) => adapter.shutdown(),
                    Err(error) => state.error = Some(error),
                }
            });
        });
    });
}

pub(crate) fn show(ctx: &egui::Context) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.open {
            return;
        }

        let mut open = state.open;
        let presenter_error =
            state
                .presenter_status
                .as_ref()
                .and_then(|status| match status.get() {
                    PresenterState::Unsupported(error) | PresenterState::Failed(error) => {
                        Some(error)
                    }
                    _ => None,
                });
        let error = state.error.clone().or(presenter_error);
        let mut requested_size = None;
        let pixels_per_point = ctx.pixels_per_point();
        egui::Window::new("Plugin Demo")
            .open(&mut open)
            .default_size([420.0, 420.0])
            .show(ctx, |ui| {
                if let Some(error) = &error {
                    ui.colored_label(egui::Color32::RED, error);
                    return;
                }
                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
                let size = [
                    (response.rect.width() * pixels_per_point).round() as u32,
                    (response.rect.height() * pixels_per_point).round() as u32,
                ];
                requested_size = Some((size, response.rect));
                let messages = state.input.update(ui, &response, pixels_per_point);
                if let Some(adapter) = &mut state.adapter {
                    if let Err(error) = adapter.send(messages) {
                        state.error = Some(error);
                    }
                }
                if let Some(status) = state.presenter_status.clone() {
                    painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
                        response.rect,
                        PresenterCallback {
                            command: PresenterCommand::Present(WebFrame {
                                size,
                                canvas_id: CANVAS_ID,
                            }),
                            status,
                        },
                    ));
                }
            });
        if state.open && !open {
            if let Some(mut adapter) = state.adapter.take() {
                adapter.shutdown();
            }
            state.input = InputAdapter::default();
            state.canvas_size = [0, 0];
            if let Some(status) = state.presenter_status.clone() {
                ctx.debug_painter()
                    .add(eframe::egui_wgpu::Callback::new_paint_callback(
                        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO),
                        PresenterCallback::<WebFrame> {
                            command: PresenterCommand::Release,
                            status,
                        },
                    ));
            }
        }
        state.open = open;

        if let Some((size, _)) = requested_size {
            if state.canvas_size != size {
                state.canvas_size = size;
            }
        }
    });
}

fn create_canvas() {
    (|| -> Option<()> {
        let document = web_sys::window()?.document()?;
        if document.get_element_by_id(CANVAS_ID).is_some() {
            return Some(());
        }
        let canvas: web_sys::HtmlCanvasElement =
            document.create_element("canvas").ok()?.dyn_into().ok()?;
        canvas.set_id(CANVAS_ID);
        let _ = canvas.style().set_property("left", "-10000px");
        let _ = canvas.style().set_property("position", "fixed");
        let _ = canvas.style().set_property("top", "0");
        let _ = canvas.style().set_property("visibility", "hidden");
        document.body()?.append_child(&canvas).ok()?;
        Some(())
    })();
}
