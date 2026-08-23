use block_plugin_api::{decode_frame, encode_frame, FrameReady, Message, ScreenLayout};
use std::cell::RefCell;
use wasm_bindgen::{prelude::*, JsCast};

use crate::{screens::Screens, web_surface::Surface};

thread_local! {
    static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
    static SESSION: RefCell<crate::session::ClientSession> = RefCell::default();
}

struct Runtime {
    screens: Screens,
    surface: Surface,
    layout: ScreenLayout,
    started: f64,
}

pub(crate) async fn start<A: crate::App>(canvas_id: String) -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("no document is available"))?;
    let canvas = document
        .get_element_by_id(&canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("no element id {canvas_id}")))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    let surface = Surface::new(canvas).await.map_err(protocol_error)?;
    SESSION.with(|current| *current.borrow_mut() = crate::session::ClientSession::default());
    RUNTIME.with(|current| {
        *current.borrow_mut() = Some(Runtime {
            screens: Screens::new::<A>(crate::Waker::default()),
            surface,
            layout: ScreenLayout::default(),
            started: now(),
        });
    });
    Ok(())
}

pub(crate) fn hello(id: &str, name: &str, version: &str) -> Result<Vec<u8>, JsValue> {
    SESSION.with(|session| {
        let client = crate::session::ClientSession::new(id, name, version);
        let frame = encode_frame(&client.hello()).map_err(protocol_error)?;
        *session.borrow_mut() = client;
        Ok(frame)
    })
}

pub(crate) fn receive(frame: Vec<u8>) -> Result<js_sys::Array, JsValue> {
    let message = decode_frame(&frame).map_err(protocol_error)?;
    let mut responses = dispatch(&message);
    responses.extend(SESSION.with(|session| session.borrow_mut().receive(message)));
    encode(responses)
}

pub(crate) fn poll() -> Result<js_sys::Array, JsValue> {
    encode(with_runtime(|runtime| runtime.screens.outbound()))
}

pub(crate) fn render() -> Result<js_sys::Array, JsValue> {
    let mut failure = None;
    let mut messages = with_runtime(|runtime| {
        let time = (now() - runtime.started) / 1000.0;
        let layout = runtime.layout.clone();
        let repaint = match runtime.surface.render(&mut runtime.screens, &layout, time) {
            Ok(repaint) => repaint,
            Err(error) => {
                failure = Some(error);
                return Vec::new();
            }
        };
        vec![Message::FrameReady(FrameReady {
            generation: layout.generation,
            damage: Vec::new(),
            synchronization_value: 0,
            repaint_after_micros: repaint.map(|delay| delay.as_micros() as u64),
            attachments: Vec::new(),
        })]
    });
    if let Some(failure) = failure {
        return Err(protocol_error(failure));
    }
    messages.extend(with_runtime(|runtime| runtime.screens.outbound()));
    encode(messages)
}

pub(crate) fn shutdown() {
    RUNTIME.with(|current| current.borrow_mut().take());
}

fn dispatch(message: &Message) -> Vec<Message> {
    with_runtime(|runtime| {
        runtime.screens.receive(message);
        let layout = runtime.screens.layout().clone();
        let mut responses = Vec::new();
        if !runtime.layout.same_placements(&layout) {
            runtime.surface.resize(&layout);
            runtime.layout = layout.clone();
            responses.push(Message::Layout(layout));
        }
        responses.extend(runtime.screens.outbound());
        responses
    })
}

fn with_runtime<R: Default>(action: impl FnOnce(&mut Runtime) -> R) -> R {
    RUNTIME.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        runtime.as_mut().map(action).unwrap_or_default()
    })
}

fn encode(messages: Vec<Message>) -> Result<js_sys::Array, JsValue> {
    messages
        .into_iter()
        .map(|message| {
            encode_frame(&message)
                .map(|frame| JsValue::from(js_sys::Uint8Array::from(frame.as_slice())))
                .map_err(protocol_error)
        })
        .collect()
}

fn now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now())
        .unwrap_or_default()
}

fn protocol_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
