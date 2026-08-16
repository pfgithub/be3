use std::cell::RefCell;

use eframe::egui;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

mod renderer;

use renderer::WasmDemoCallback;

const WASM_DEMO_URL: &str = "/wasm_demo.js";
const CANVAS_ID: &str = "wasm-demo-canvas";

#[wasm_bindgen(inline_js = "
export async function run_wasm_demo(url, canvas_id) {
    const module = await import(url);
    await module.default();
    await module.start(canvas_id);
}
")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn run_wasm_demo(url: &str, canvas_id: &str) -> Result<(), JsValue>;
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[derive(Default)]
struct State {
    open: bool,
    started: bool,
    render_available: bool,
    error: Option<String>,
    canvas_size: [u32; 2],
}

pub(crate) fn install(creation_context: &eframe::CreationContext<'_>) {
    let render_available = renderer::install(creation_context);
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.render_available = render_available;
        if !render_available {
            state.error = Some("wgpu is not available in this build.".to_owned());
        }
    });
}

pub(crate) fn open() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.open = true;
        if state.started || !state.render_available {
            return;
        }
        state.started = true;
        create_canvas();
        wasm_bindgen_futures::spawn_local(async {
            if let Err(error) = run_wasm_demo(WASM_DEMO_URL, CANVAS_ID).await {
                let message = error.as_string().unwrap_or_else(|| format!("{error:?}"));
                web_sys::console::error_1(&error);
                STATE.with(|state| state.borrow_mut().error = Some(message));
            }
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
        let error = state.error.clone();
        let mut requested_size = None;
        egui::Window::new("Wasm Demo")
            .open(&mut open)
            .default_size([420.0, 420.0])
            .show(ctx, |ui| {
                if let Some(error) = &error {
                    ui.colored_label(egui::Color32::RED, error);
                    return;
                }
                let pixels_per_point = ui.ctx().pixels_per_point();
                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), egui::Sense::hover());
                let size = [
                    (response.rect.width() * pixels_per_point).round() as u32,
                    (response.rect.height() * pixels_per_point).round() as u32,
                ];
                requested_size = Some(size);
                painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
                    response.rect,
                    WasmDemoCallback {
                        size,
                        canvas_id: CANVAS_ID,
                    },
                ));
            });
        state.open = open;

        if let Some(size) = requested_size {
            if state.canvas_size != size {
                resize_canvas(size, pixels_per_point);
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

fn resize_canvas(size: [u32; 2], pixels_per_point: f32) {
    (|| -> Option<()> {
        let canvas: web_sys::HtmlCanvasElement = web_sys::window()?
            .document()?
            .get_element_by_id(CANVAS_ID)?
            .dyn_into()
            .ok()?;
        let style = canvas.style();
        let _ = style.set_property("width", &format!("{}px", size[0] as f32 / pixels_per_point));
        let _ = style.set_property(
            "height",
            &format!("{}px", size[1] as f32 / pixels_per_point),
        );
        Some(())
    })();
}
