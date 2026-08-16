use std::cell::RefCell;

use eframe::egui;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

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
}

/// Nothing to do at startup: the wasm-demo module and its wgpu device are
/// loaded lazily, the first time the debug window opens.
pub(crate) fn install(_creation_context: &eframe::CreationContext<'_>) {}

/// Opens the wasm-demo debug window, loading and starting the wasm-demo
/// module the first time this is called. wasm-demo sets up its own wgpu
/// device against its own canvas and drives its own render loop from there;
/// this side just has to load it once and keep that canvas positioned over
/// the window's content area for as long as it stays open.
pub(crate) fn open() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.open = true;
        if !state.started {
            state.started = true;
            create_canvas();
            wasm_bindgen_futures::spawn_local(async {
                if let Err(error) = run_wasm_demo(WASM_DEMO_URL, CANVAS_ID).await {
                    web_sys::console::error_1(&error);
                }
            });
        }
    });
}

/// Draws the wasm-demo debug window, if open, and keeps the wasm-demo canvas
/// positioned over its content area (or hidden while the window is closed).
pub(crate) fn show(ctx: &egui::Context) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.open {
            hide_canvas();
            return;
        }

        let mut open = state.open;
        let mut content_rect = None;
        egui::Window::new("Wasm Demo")
            .open(&mut open)
            .default_size([420.0, 420.0])
            .show(ctx, |ui| {
                let (rect, _response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
                content_rect = Some(rect);
            });
        state.open = open;

        match (state.open, content_rect) {
            (true, Some(rect)) => position_canvas(rect),
            (true, None) => {}
            (false, _) => hide_canvas(),
        }
    });
}

fn canvas_element() -> Option<web_sys::HtmlCanvasElement> {
    web_sys::window()?
        .document()?
        .get_element_by_id(CANVAS_ID)?
        .dyn_into()
        .ok()
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
        let style = canvas.style();
        let _ = style.set_property("position", "absolute");
        let _ = style.set_property("z-index", "1");
        let _ = style.set_property("display", "none");
        document.body()?.append_child(&canvas).ok()?;
        Some(())
    })();
}

fn position_canvas(rect: egui::Rect) {
    let Some(canvas) = canvas_element() else {
        return;
    };
    let style = canvas.style();
    let _ = style.set_property("display", "block");
    let _ = style.set_property("left", &format!("{}px", rect.min.x));
    let _ = style.set_property("top", &format!("{}px", rect.min.y));
    let _ = style.set_property("width", &format!("{}px", rect.width()));
    let _ = style.set_property("height", &format!("{}px", rect.height()));
}

fn hide_canvas() {
    let Some(canvas) = canvas_element() else {
        return;
    };
    let _ = canvas.style().set_property("display", "none");
}
