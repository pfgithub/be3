use block_plugin_api::{encode_frame, Message};
use eframe::egui;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{prelude::*, JsCast};

const WORKER_SOURCE: &str = r#"
// The worker one plugin runs in.
//
// A plugin is the same wasm every other platform runs, so the worker hands it
// to plugin.js, which stands block-gpu-shim up on this worker's canvas and
// answers the plugin's gpu abi from it. Stepping is scheduled rather than
// immediate: a step that produced something schedules the next one, and a
// quiet plugin stops until the host says something.
let plugin = null;
const queued = [];
let scheduled = false;

function post(frames) {
    self.postMessage({ kind: "frames", frames });
}

function fail(error) {
    self.postMessage({ kind: "error", message: String((error && error.message) || error) });
}

function schedule() {
    if (scheduled || plugin === null) {
        return;
    }
    scheduled = true;
    setTimeout(step, 0);
}

function drain() {
    const frames = plugin.collect();
    if (frames.length > 0) {
        post(frames);
        schedule();
    }
    const failure = plugin.failure();
    if (failure) {
        fail(failure);
    }
}

function step() {
    scheduled = false;
    if (plugin === null) {
        return;
    }
    try {
        plugin.step();
        drain();
    } catch (error) {
        plugin = null;
        fail(error);
    }
}

self.onmessage = async (event) => {
    const data = event.data;
    try {
        if (data.kind === "start") {
            const bootstrap = await import(new URL("./plugin.js", data.url).href);
            plugin = await bootstrap.boot(
                new URL("./block_gpu_shim.js", data.url).href,
                data.url,
                data.canvas,
            );
            drain();
            for (const frame of queued.splice(0)) plugin.deliver(frame);
            schedule();
        } else if (data.kind === "frames") {
            for (const frame of data.frames) {
                if (plugin) plugin.deliver(frame);
                else queued.push(frame);
            }
            schedule();
        } else if (data.kind === "shutdown") {
            if (plugin) plugin.shutdown();
            self.close();
        }
    } catch (error) {
        fail(error);
    }
};
"#;

#[derive(Default)]
struct Inbox {
    frames: Vec<Vec<u8>>,
    error: Option<String>,
}

pub(super) struct WebProtocolAdapter {
    worker: web_sys::Worker,
    inbox: Rc<RefCell<Inbox>>,
    received: Vec<Message>,
    frames: u64,
    spoken: bool,
    _onmessage: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

impl WebProtocolAdapter {
    pub(super) fn start(
        url: &str,
        canvas: &web_sys::HtmlCanvasElement,
        context: &egui::Context,
    ) -> Result<Self, String> {
        let offscreen = canvas
            .transfer_control_to_offscreen()
            .map_err(|_| "the plugin canvas could not be handed to its worker".to_owned())?;
        let worker = spawn()?;
        let inbox = Rc::new(RefCell::new(Inbox::default()));
        let onmessage = listen(&worker, Rc::clone(&inbox), context.clone());
        let message = js_sys::Object::new();
        set(&message, "kind", &"start".into());
        set(&message, "url", &absolute(url).into());
        set(&message, "canvas", &offscreen);
        let transfer = js_sys::Array::of1(&offscreen);
        worker
            .post_message_with_transfer(&message, &transfer)
            .map_err(|_| "the plugin worker could not be started".to_owned())?;
        Ok(Self {
            worker,
            inbox,
            received: Vec::new(),
            frames: 0,
            spoken: false,
            _onmessage: onmessage,
        })
    }

    pub(super) fn frames(&self) -> u64 {
        self.frames
    }

    pub(super) fn running(&self) -> bool {
        self.spoken
    }

    pub(super) fn send(&mut self, messages: Vec<Message>) -> Result<(), String> {
        let frames = js_sys::Array::new();
        for message in messages {
            let frame = encode_frame(&message).map_err(|error| error.to_string())?;
            frames.push(&js_sys::Uint8Array::from(frame.as_slice()).into());
        }
        if frames.length() == 0 {
            return Ok(());
        }
        let message = js_sys::Object::new();
        set(&message, "kind", &"frames".into());
        set(&message, "frames", &frames);
        self.worker
            .post_message(&message)
            .map_err(|_| "the plugin worker stopped listening".to_owned())
    }

    pub(super) fn poll(&mut self) -> Result<(), String> {
        let (frames, error) = {
            let mut inbox = self.inbox.borrow_mut();
            (std::mem::take(&mut inbox.frames), inbox.error.take())
        };
        if let Some(error) = error {
            return Err(error);
        }
        for frame in frames {
            let message = decode(&frame)?;
            self.spoken = true;
            if matches!(message, Message::FrameReady(_)) {
                self.frames += 1;
            }
            self.received.push(message);
        }
        Ok(())
    }

    pub(super) fn take_received(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.received)
    }

    pub(super) fn shutdown(&mut self) {
        let message = js_sys::Object::new();
        set(&message, "kind", &"shutdown".into());
        let _ = self.worker.post_message(&message);
        self.worker.terminate();
    }
}

fn listen(
    worker: &web_sys::Worker,
    inbox: Rc<RefCell<Inbox>>,
    context: egui::Context,
) -> Closure<dyn FnMut(web_sys::MessageEvent)> {
    let onmessage = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
        let data = event.data();
        let kind = get(&data, "kind").as_string().unwrap_or_default();
        let mut inbox = inbox.borrow_mut();
        match kind.as_str() {
            "frames" => {
                let frames = js_sys::Array::from(&get(&data, "frames"));
                for frame in frames.iter() {
                    inbox.frames.push(js_sys::Uint8Array::new(&frame).to_vec());
                }
            }
            _ => {
                inbox.error = Some(
                    get(&data, "message")
                        .as_string()
                        .unwrap_or_else(|| "The plugin worker failed.".to_owned()),
                );
            }
        }
        context.request_repaint();
    }) as Box<dyn FnMut(web_sys::MessageEvent)>);
    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage
}

fn spawn() -> Result<web_sys::Worker, String> {
    let source = js_sys::Array::of1(&WORKER_SOURCE.into());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("text/javascript");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&source, &options)
        .map_err(|_| "the plugin worker could not be assembled".to_owned())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "the plugin worker could not be addressed".to_owned())?;
    let options = web_sys::WorkerOptions::new();
    options.set_type(web_sys::WorkerType::Module);
    let worker = web_sys::Worker::new_with_options(&url, &options)
        .map_err(|_| "the plugin worker could not be started".to_owned());
    let _ = web_sys::Url::revoke_object_url(&url);
    worker
}

fn absolute(url: &str) -> String {
    let base = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.base_uri().ok().flatten())
        .unwrap_or_default();
    web_sys::Url::new_with_base(url, &base)
        .map(|url| url.href())
        .unwrap_or_else(|_| url.to_owned())
}

fn set(object: &js_sys::Object, key: &str, value: &JsValue) {
    let _ = js_sys::Reflect::set(object, &key.into(), value);
}

fn get(object: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(object, &key.into()).unwrap_or(JsValue::UNDEFINED)
}

fn decode(frame: &[u8]) -> Result<Message, String> {
    block_plugin_api::decode_frame(frame).map_err(|error| error.to_string())
}
