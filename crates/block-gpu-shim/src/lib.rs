#![cfg(target_arch = "wasm32")]

mod exports;
mod surface;

use std::{cell::RefCell, collections::VecDeque};

use block_gpu_host::Gpu;
use wasm_bindgen::prelude::*;

use surface::Canvas;

thread_local! {
    static SHIM: RefCell<Option<Shim>> = const { RefCell::new(None) };
}

struct Shim {
    gpu: Gpu,
    canvas: Canvas,
    inbox: VecDeque<Vec<u8>>,
    outbox: Vec<Vec<u8>>,
    scratch: Vec<u8>,
    started: f64,
    failure: Option<String>,
}

impl Shim {
    fn report(&mut self, message: String) {
        self.gpu.report(message);
    }
}

fn with<R>(act: impl FnOnce(&mut Shim) -> R, absent: R) -> R {
    SHIM.with(|shim| match shim.borrow_mut().as_mut() {
        Some(shim) => act(shim),
        None => absent,
    })
}

#[wasm_bindgen]
pub async fn start(canvas: JsValue) -> Result<(), JsValue> {
    let canvas: web_sys::OffscreenCanvas = canvas.dyn_into()?;
    let (canvas, device, queue) = Canvas::open(canvas)
        .await
        .map_err(|error| JsValue::from_str(&error))?;
    let shim = Shim {
        gpu: Gpu::new(device, queue),
        canvas,
        inbox: VecDeque::new(),
        outbox: Vec::new(),
        scratch: Vec::new(),
        started: now(),
        failure: None,
    };
    SHIM.with(|current| *current.borrow_mut() = Some(shim));
    Ok(())
}

#[wasm_bindgen]
pub fn deliver(frame: &[u8]) {
    with(|shim| shim.inbox.push_back(frame.to_vec()), ());
}

#[wasm_bindgen]
pub fn collect() -> js_sys::Array {
    let frames = js_sys::Array::new();
    with(
        |shim| {
            for frame in std::mem::take(&mut shim.outbox) {
                frames.push(&js_sys::Uint8Array::from(frame.as_slice()).into());
            }
        },
        (),
    );
    frames
}

#[wasm_bindgen]
pub fn failure() -> Option<String> {
    with(
        |shim| shim.failure.take().or_else(|| shim.gpu.take_error()),
        None,
    )
}

fn now() -> f64 {
    js_sys::Date::now()
}
