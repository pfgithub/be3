use std::cell::{Cell, RefCell};

use block_plugin_api::{decode_frame, encode_frame, ErrorCode, Message, ProtocolError};
use wasm_bindgen::{prelude::*, JsCast};

use crate::{runtime::Runtime, web::surface, Waker};

pub(crate) type Attachment = ();

const TICK_SLACK_MILLISECONDS: f64 = 1.0;

thread_local! {
    static PLUGIN: RefCell<Option<Plugin>> = const { RefCell::new(None) };
    static PENDING: RefCell<Vec<Message>> = const { RefCell::new(Vec::new()) };
    static WOKEN: Cell<bool> = const { Cell::new(false) };
    static SCHEDULED: Cell<Option<f64>> = const { Cell::new(None) };
}

struct Plugin {
    runtime: Runtime,
    post: js_sys::Function,
    started: f64,
}

pub(crate) async fn start<A: crate::App>(
    canvas: JsValue,
    post: js_sys::Function,
    id: &str,
    name: &str,
    version: &str,
) -> Result<(), JsValue> {
    let canvas: web_sys::OffscreenCanvas = canvas.dyn_into()?;
    surface::initialize(canvas).await.map_err(failure)?;
    let runtime = Runtime::new::<A>(id, name, version, waker());
    let hello = runtime.hello();
    let plugin = Plugin {
        runtime,
        post,
        started: now(),
    };
    post_messages(&plugin.post, vec![hello])?;
    PLUGIN.with(|current| *current.borrow_mut() = Some(plugin));
    Ok(())
}

pub(crate) fn receive(frame: Vec<u8>) -> Result<(), JsValue> {
    let message = decode_frame(&frame).map_err(|error| failure(format!("{error:?}")))?;
    PENDING.with(|pending| pending.borrow_mut().push(message));
    schedule(0.0);
    Ok(())
}

pub(crate) fn shutdown() {
    PLUGIN.with(|plugin| plugin.borrow_mut().take());
    PENDING.with(|pending| pending.borrow_mut().clear());
}

fn waker() -> Waker {
    Waker::new(|| {
        WOKEN.with(|woken| woken.set(true));
        schedule(0.0);
    })
}

fn schedule(delay: f64) {
    let delay = delay.max(0.0);
    let at = now() + delay;
    if SCHEDULED
        .with(|scheduled| scheduled.get())
        .is_some_and(|already| already <= at + TICK_SLACK_MILLISECONDS)
    {
        return;
    }
    SCHEDULED.with(|scheduled| scheduled.set(Some(at)));
    let callback = Closure::once_into_js(tick);
    let Some(global) = global() else {
        return;
    };
    let _ = global.set_timeout_with_callback_and_timeout_and_arguments_0(
        callback.unchecked_ref(),
        delay as i32,
    );
}

fn tick() {
    SCHEDULED.with(|scheduled| scheduled.set(None));
    let batch = PENDING.with(|pending| std::mem::take(&mut *pending.borrow_mut()));
    let draw = WOKEN.with(|woken| woken.replace(false));
    let outcome = PLUGIN.with(|plugin| {
        let mut plugin = plugin.borrow_mut();
        let plugin = plugin.as_mut()?;
        let phase = (now() - plugin.started) / 1000.0;
        Some(match plugin.runtime.step(batch, draw, phase) {
            Ok(step) => {
                let sent = post_messages(
                    &plugin.post,
                    step.outbound.into_iter().map(|out| out.message).collect(),
                );
                match sent {
                    Ok(()) => (step.repaint, step.closed),
                    Err(_) => (None, true),
                }
            }
            Err(error) => {
                let _ = post_messages(&plugin.post, vec![protocol_error(error)]);
                (None, true)
            }
        })
    });
    match outcome {
        Some((_, true)) => shutdown(),
        Some((Some(delay), false)) => schedule(delay.as_secs_f64() * 1000.0),
        _ => {}
    }
}

fn post_messages(post: &js_sys::Function, messages: Vec<Message>) -> Result<(), JsValue> {
    if messages.is_empty() {
        return Ok(());
    }
    let frames = js_sys::Array::new();
    for message in messages {
        let frame = encode_frame(&message).map_err(|error| failure(error.to_string()))?;
        frames.push(&js_sys::Uint8Array::from(frame.as_slice()).into());
    }
    post.call1(&JsValue::NULL, &frames)?;
    Ok(())
}

fn protocol_error(message: String) -> Message {
    Message::Error(ProtocolError {
        request_id: None,
        code: ErrorCode::Internal,
        message,
    })
}

fn global() -> Option<web_sys::DedicatedWorkerGlobalScope> {
    js_sys::global().dyn_into().ok()
}

fn now() -> f64 {
    global()
        .and_then(|global| global.performance())
        .map(|performance| performance.now())
        .unwrap_or_default()
}

fn failure(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
