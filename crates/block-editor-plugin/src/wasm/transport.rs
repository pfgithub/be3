use std::cell::{Cell, RefCell};

use block_plugin_api::{decode_frame, encode_frame, ErrorCode, Message, ProtocolError};

use crate::{runtime::Runtime, wasm::host, wasm::surface, Waker};

pub(crate) type Attachment = ();

thread_local! {
    static PLUGIN: RefCell<Option<Runtime>> = const { RefCell::new(None) };
    static WOKEN: Cell<bool> = const { Cell::new(false) };
    static STARTED: Cell<f64> = const { Cell::new(0.0) };
}

pub(crate) fn start<A: crate::App>(id: &str, name: &str, version: &str) -> Result<(), String> {
    surface::initialize()?;
    let runtime = Runtime::new::<A>(id, name, version, waker());
    post(vec![runtime.hello()])?;
    STARTED.with(|started| started.set(host::now()));
    PLUGIN.with(|plugin| *plugin.borrow_mut() = Some(runtime));
    Ok(())
}

pub(crate) fn step() -> Result<(), String> {
    let mut batch = Vec::new();
    while let Some(frame) = host::receive() {
        batch.push(decode_frame(&frame).map_err(|error| format!("{error:?}"))?);
    }
    let woken = WOKEN.with(|woken| woken.replace(false));
    let phase = host::now() - STARTED.with(Cell::get);
    let outcome = PLUGIN.with(|plugin| {
        let mut plugin = plugin.borrow_mut();
        let Some(runtime) = plugin.as_mut() else {
            return Ok(false);
        };
        match runtime.step(batch, woken, phase) {
            Ok(step) => {
                let messages = step.outbound.into_iter().map(|out| out.message).collect();
                post(messages)?;
                Ok(step.closed)
            }
            Err(error) => {
                let _ = post(vec![protocol_error(error.clone())]);
                Err(error)
            }
        }
    });
    match outcome {
        Ok(true) => {
            shutdown();
            Ok(())
        }
        Ok(false) => Ok(()),
        Err(error) => {
            shutdown();
            Err(error)
        }
    }
}

pub(crate) fn initialize_storage(size: usize, align: usize) {
    wasi_threads::initialize_main_thread_storage(size, align);
}

pub(crate) fn shutdown() {
    PLUGIN.with(|plugin| plugin.borrow_mut().take());
}

fn waker() -> Waker {
    let plugin_thread = std::thread::current().id();
    Waker::new(move || {
        if std::thread::current().id() == plugin_thread {
            WOKEN.with(|woken| woken.set(true));
        }
    })
}

fn post(messages: Vec<Message>) -> Result<(), String> {
    for message in messages {
        let frame = encode_frame(&message).map_err(|error| error.to_string())?;
        host::send(&frame);
    }
    Ok(())
}

fn protocol_error(message: String) -> Message {
    Message::Error(ProtocolError {
        request_id: None,
        code: ErrorCode::Internal,
        message,
    })
}
