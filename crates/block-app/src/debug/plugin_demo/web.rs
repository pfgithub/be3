use std::cell::RefCell;

use eframe::egui;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

mod renderer;

use super::input::InputAdapter;
use block_plugin_api::{InputEvent, Message, PointerButton, WheelUnit};
use renderer::PluginDemoCallback;

const PLUGIN_DEMO_URL: &str = "/plugin_demo.js";
const CANVAS_ID: &str = "plugin-demo-canvas";

#[wasm_bindgen(inline_js = "
export async function run_plugin_demo(url, canvas_id) {
    const module = await import(url);
    await module.default();
    await module.start(canvas_id);
}

export function plugin_demo_pointer(canvas_id, kind, x, y, button, buttons, ctrl, shift, alt, meta) {
    const canvas = document.getElementById(canvas_id);
    const rect = canvas.getBoundingClientRect();
    const event = new PointerEvent(kind, {
        bubbles: true,
        clientX: rect.left + x,
        clientY: rect.top + y,
        button,
        buttons,
        ctrlKey: ctrl,
        shiftKey: shift,
        altKey: alt,
        metaKey: meta,
        pointerId: 1,
        pointerType: 'mouse',
    });
    queueMicrotask(() => {
        (kind === 'pointerdown' ? canvas : document).dispatchEvent(event);
    });
}

export function plugin_demo_wheel(canvas_id, x, y, delta_x, delta_y, ctrl, shift, alt, meta) {
    const canvas = document.getElementById(canvas_id);
    const rect = canvas.getBoundingClientRect();
    const event = new WheelEvent('wheel', {
        bubbles: true,
        clientX: rect.left + x,
        clientY: rect.top + y,
        deltaX: delta_x,
        deltaY: delta_y,
        ctrlKey: ctrl,
        shiftKey: shift,
        altKey: alt,
        metaKey: meta,
    });
    queueMicrotask(() => canvas.dispatchEvent(event));
}
")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn run_plugin_demo(url: &str, canvas_id: &str) -> Result<(), JsValue>;

    fn plugin_demo_pointer(
        canvas_id: &str,
        kind: &str,
        x: f32,
        y: f32,
        button: i16,
        buttons: u16,
        ctrl: bool,
        shift: bool,
        alt: bool,
        meta: bool,
    );

    fn plugin_demo_wheel(
        canvas_id: &str,
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        ctrl: bool,
        shift: bool,
        alt: bool,
        meta: bool,
    );
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
    input: InputAdapter,
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
            if let Err(error) = run_plugin_demo(PLUGIN_DEMO_URL, CANVAS_ID).await {
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
                forward_input(
                    state.input.update(ui, &response, pixels_per_point),
                    ui,
                    &response,
                );
                painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
                    response.rect,
                    PluginDemoCallback {
                        size,
                        canvas_id: CANVAS_ID,
                    },
                ));
            });
        state.open = open;

        if let Some((size, _)) = requested_size {
            if state.canvas_size != size {
                state.canvas_size = size;
            }
            resize_canvas(size, pixels_per_point);
        }
    });
}

fn forward_input(messages: Vec<Message>, ui: &egui::Ui, response: &egui::Response) {
    for message in messages {
        let Message::Input(input) = message else {
            continue;
        };
        for event in input.events {
            match event {
                InputEvent::PointerMoved { x, y } => {
                    let buttons = ui.input(|input| pointer_buttons(&input.pointer));
                    plugin_demo_pointer(
                        CANVAS_ID,
                        "mousemove",
                        x,
                        y,
                        -1,
                        buttons,
                        false,
                        false,
                        false,
                        false,
                    );
                }
                InputEvent::PointerButton {
                    button,
                    pressed,
                    x,
                    y,
                } => {
                    let buttons = ui.input(|input| pointer_buttons(&input.pointer));
                    plugin_demo_pointer(
                        CANVAS_ID,
                        if pressed { "pointerdown" } else { "pointerup" },
                        x,
                        y,
                        pointer_button(button),
                        buttons,
                        false,
                        false,
                        false,
                        false,
                    );
                }
                InputEvent::Wheel { x, y, unit } if response.hovered() => {
                    let position = ui
                        .input(|input| input.pointer.hover_pos())
                        .unwrap_or(response.rect.center())
                        - response.rect.min;
                    plugin_demo_wheel(
                        CANVAS_ID,
                        position.x,
                        position.y,
                        -x * wheel_scale(unit),
                        -y * wheel_scale(unit),
                        false,
                        false,
                        false,
                        false,
                    );
                }
                _ => {}
            }
        }
    }
}

fn pointer_button(button: PointerButton) -> i16 {
    match button {
        PointerButton::Primary => 0,
        PointerButton::Middle => 1,
        PointerButton::Secondary => 2,
        PointerButton::Back => 3,
        PointerButton::Forward => 4,
        PointerButton::Other(button) => button as i16,
    }
}

fn wheel_scale(unit: WheelUnit) -> f32 {
    match unit {
        WheelUnit::Pixels => 1.0,
        WheelUnit::Lines => 40.0,
        WheelUnit::Pages => 400.0,
    }
}

fn pointer_buttons(pointer: &egui::PointerState) -> u16 {
    u16::from(pointer.button_down(egui::PointerButton::Primary))
        | u16::from(pointer.button_down(egui::PointerButton::Secondary)) << 1
        | u16::from(pointer.button_down(egui::PointerButton::Middle)) << 2
        | u16::from(pointer.button_down(egui::PointerButton::Extra1)) << 3
        | u16::from(pointer.button_down(egui::PointerButton::Extra2)) << 4
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
