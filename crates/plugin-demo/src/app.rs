use block_plugin_api::{
    decode_frame, encode_frame, InputEvent, Message, Modifiers, PointerButton, WheelUnit,
};
use std::cell::RefCell;
use wasm_bindgen::{prelude::*, JsCast};

thread_local! {
    static RUNNER: RefCell<Option<eframe::WebRunner>> = const { RefCell::new(None) };
    static SESSION: RefCell<crate::native::ClientSession> = RefCell::default();
    static CANVAS_ID: RefCell<String> = const { RefCell::new(String::new()) };
    static MODIFIERS: RefCell<Modifiers> = RefCell::default();
    static BUTTONS: RefCell<u16> = const { RefCell::new(0) };
}

impl eframe::App for crate::demo::Demo {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

#[wasm_bindgen(inline_js = "
export function plugin_resize(canvasId, width, height) {
    const canvas = document.getElementById(canvasId);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
}

export function plugin_pointer(canvasId, kind, x, y, button, buttons, alt, control, shift, command) {
    const canvas = document.getElementById(canvasId);
    const rect = canvas.getBoundingClientRect();
    const event = new PointerEvent(kind, {
        bubbles: true, clientX: rect.left + x, clientY: rect.top + y, button, buttons,
        altKey: alt, ctrlKey: control, shiftKey: shift, metaKey: command,
        pointerId: 1, pointerType: 'mouse',
    });
    queueMicrotask(() => (kind === 'pointerdown' ? canvas : document).dispatchEvent(event));
}

export function plugin_wheel(canvasId, x, y, unit, alt, control, shift, command) {
    const canvas = document.getElementById(canvasId);
    const event = new WheelEvent('wheel', {
        bubbles: true, deltaX: -x * unit, deltaY: -y * unit, deltaMode: 0,
        altKey: alt, ctrlKey: control, shiftKey: shift, metaKey: command,
    });
    queueMicrotask(() => canvas.dispatchEvent(event));
}

export function plugin_key(canvasId, logical, pressed, repeat, alt, control, shift, command) {
    const canvas = document.getElementById(canvasId);
    const event = new KeyboardEvent(pressed ? 'keydown' : 'keyup', {
        bubbles: true, key: logical, repeat,
        altKey: alt, ctrlKey: control, shiftKey: shift, metaKey: command,
    });
    queueMicrotask(() => canvas.dispatchEvent(event));
}

export function plugin_text(canvasId, value) {
    const canvas = document.getElementById(canvasId);
    const event = new InputEvent('input', { bubbles: true, data: value, inputType: 'insertText' });
    queueMicrotask(() => canvas.dispatchEvent(event));
}

export function plugin_focus(canvasId, focused) {
    const canvas = document.getElementById(canvasId);
    queueMicrotask(() => focused ? canvas.focus() : canvas.blur());
}
")]
extern "C" {
    fn plugin_resize(canvas_id: &str, width: f32, height: f32);
    fn plugin_pointer(
        canvas_id: &str,
        kind: &str,
        x: f32,
        y: f32,
        button: i16,
        buttons: u16,
        alt: bool,
        control: bool,
        shift: bool,
        command: bool,
    );
    fn plugin_wheel(
        canvas_id: &str,
        x: f32,
        y: f32,
        unit: f32,
        alt: bool,
        control: bool,
        shift: bool,
        command: bool,
    );
    fn plugin_key(
        canvas_id: &str,
        logical: &str,
        pressed: bool,
        repeat: bool,
        alt: bool,
        control: bool,
        shift: bool,
        command: bool,
    );
    fn plugin_text(canvas_id: &str, value: &str);
    fn plugin_focus(canvas_id: &str, focused: bool);
}

#[wasm_bindgen]
pub async fn start(canvas_id: String) -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("no document is available"))?;
    let canvas = document
        .get_element_by_id(&canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("no element id {canvas_id}")))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    let runner = eframe::WebRunner::new();
    runner
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|_| Ok(Box::<crate::demo::Demo>::default())),
        )
        .await?;
    CANVAS_ID.with(|current| *current.borrow_mut() = canvas_id);
    SESSION.with(|session| *session.borrow_mut() = crate::native::ClientSession::default());
    RUNNER.with(|current| current.replace(Some(runner)));
    Ok(())
}

#[wasm_bindgen]
pub fn hello() -> Result<Vec<u8>, JsValue> {
    SESSION.with(|session| encode_frame(&session.borrow().hello()).map_err(protocol_error))
}

#[wasm_bindgen]
pub fn receive(frame: Vec<u8>) -> Result<js_sys::Array, JsValue> {
    let message = decode_frame(&frame).map_err(protocol_error)?;
    dispatch(&message);
    SESSION.with(|session| {
        session
            .borrow_mut()
            .receive(message)
            .into_iter()
            .map(|message| {
                encode_frame(&message)
                    .map(|frame| JsValue::from(js_sys::Uint8Array::from(frame.as_slice())))
                    .map_err(protocol_error)
            })
            .collect()
    })
}

#[wasm_bindgen]
pub fn shutdown() {
    RUNNER.with(|current| {
        if let Some(runner) = current.borrow_mut().take() {
            runner.destroy();
        }
    });
}

fn dispatch(message: &Message) {
    match message {
        Message::CreateViewport(viewport) => resize(&viewport.metrics),
        Message::ResizeViewport(metrics) => resize(metrics),
        Message::Input(batch) => {
            for event in &batch.events {
                input(event);
            }
        }
        _ => {}
    }
}

fn resize(metrics: &block_plugin_api::ViewportMetrics) {
    CANVAS_ID.with(|id| plugin_resize(&id.borrow(), metrics.logical_width, metrics.logical_height));
}

fn input(event: &InputEvent) {
    CANVAS_ID.with(|canvas_id| {
        MODIFIERS.with(|modifiers| {
            BUTTONS.with(|buttons| {
                let canvas_id = canvas_id.borrow();
                let mut modifiers = modifiers.borrow_mut();
                let mut buttons = buttons.borrow_mut();
                match event {
                    InputEvent::PointerMoved { x, y } => {
                        pointer(&canvas_id, "pointermove", *x, *y, -1, *buttons, *modifiers)
                    }
                    InputEvent::PointerButton {
                        button,
                        pressed,
                        x,
                        y,
                    } => {
                        let index = pointer_button_index(*button);
                        let mask = 1 << index;
                        if *pressed {
                            *buttons |= mask
                        } else {
                            *buttons &= !mask
                        }
                        pointer(
                            &canvas_id,
                            if *pressed { "pointerdown" } else { "pointerup" },
                            *x,
                            *y,
                            index,
                            *buttons,
                            *modifiers,
                        );
                    }
                    InputEvent::Wheel { x, y, unit } => plugin_wheel(
                        &canvas_id,
                        *x,
                        *y,
                        wheel_scale(*unit),
                        modifiers.alt,
                        modifiers.control,
                        modifiers.shift,
                        modifiers.command,
                    ),
                    InputEvent::Key {
                        logical,
                        pressed,
                        repeat,
                        ..
                    } => plugin_key(
                        &canvas_id,
                        logical,
                        *pressed,
                        *repeat,
                        modifiers.alt,
                        modifiers.control,
                        modifiers.shift,
                        modifiers.command,
                    ),
                    InputEvent::Text(text) => plugin_text(&canvas_id, text),
                    InputEvent::Modifiers(next) => *modifiers = *next,
                    InputEvent::Focus(focused) => plugin_focus(&canvas_id, *focused),
                }
            });
        });
    });
}

fn pointer(
    canvas_id: &str,
    kind: &str,
    x: f32,
    y: f32,
    button: i16,
    buttons: u16,
    modifiers: Modifiers,
) {
    plugin_pointer(
        canvas_id,
        kind,
        x,
        y,
        button,
        buttons,
        modifiers.alt,
        modifiers.control,
        modifiers.shift,
        modifiers.command,
    );
}

fn pointer_button_index(button: PointerButton) -> i16 {
    match button {
        PointerButton::Primary => 0,
        PointerButton::Middle => 1,
        PointerButton::Secondary => 2,
        PointerButton::Back => 3,
        PointerButton::Forward => 4,
        PointerButton::Other(index) => index as i16,
    }
}

fn wheel_scale(unit: WheelUnit) -> f32 {
    match unit {
        WheelUnit::Pixels => 1.0,
        WheelUnit::Lines => 40.0,
        WheelUnit::Pages => 800.0,
    }
}

fn protocol_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
