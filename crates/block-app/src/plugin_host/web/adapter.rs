use block_plugin_api::{
    encode_frame, Capability, EditorMessage, HostSession, Message, QueueError, RegionSize,
    ScreenLayout, SessionState, SurfaceMechanism, TunnelMessage,
};
use std::time::Duration;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = "
const plugins = new Map();

export async function web_plugin_start(url, canvasId) {
    const module = await import(new URL(url, document.baseURI).href);
    await module.default();
    await module.start(canvasId);
    plugins.set(canvasId, module);
    return module.hello();
}

export function web_plugin_send(canvasId, frames) {
    const module = plugins.get(canvasId);
    if (!module) throw new Error('the web plugin is not running');
    const responses = [];
    for (const frame of frames) {
        for (const response of module.receive(frame)) {
            responses.push(response);
        }
    }
    return responses;
}

export function web_plugin_poll(canvasId) {
    const module = plugins.get(canvasId);
    if (!module) return [];
    return module.poll();
}

export function web_plugin_render(canvasId) {
    const module = plugins.get(canvasId);
    if (!module) return [];
    return module.render();
}

export function web_plugin_shutdown(canvasId) {
    const module = plugins.get(canvasId);
    if (module) {
        module.shutdown();
        plugins.delete(canvasId);
    }
}
")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn web_plugin_start(url: &str, canvas_id: &str) -> Result<js_sys::Uint8Array, JsValue>;
    #[wasm_bindgen(catch)]
    fn web_plugin_send(canvas_id: &str, frames: js_sys::Array) -> Result<js_sys::Array, JsValue>;
    #[wasm_bindgen(catch)]
    fn web_plugin_poll(canvas_id: &str) -> Result<js_sys::Array, JsValue>;
    #[wasm_bindgen(catch)]
    fn web_plugin_render(canvas_id: &str) -> Result<js_sys::Array, JsValue>;
    fn web_plugin_shutdown(canvas_id: &str);
}

pub(super) struct WebProtocolAdapter {
    canvas_id: String,
    session: HostSession,
    client_messages: Vec<TunnelMessage>,
    editor_messages: Vec<EditorMessage>,
    layout: Option<ScreenLayout>,
    region_sizes: Vec<RegionSize>,
    repaint: Option<Duration>,
}

impl WebProtocolAdapter {
    pub(super) async fn start(url: String, canvas_id: String) -> Result<Self, String> {
        let hello = web_plugin_start(&url, &canvas_id)
            .await
            .map_err(js_error)?
            .to_vec();
        let mut session = HostSession::new(
            "BE3 web host",
            vec![
                Capability::Input,
                Capability::Lifecycle,
                Capability::Surface(SurfaceMechanism::WebExternalImage),
            ],
        );
        session.start(now());
        session.receive(decode(&hello)?, now());
        let mut adapter = Self {
            canvas_id,
            session,
            client_messages: Vec::new(),
            editor_messages: Vec::new(),
            layout: None,
            region_sizes: Vec::new(),
            repaint: None,
        };
        adapter.flush()?;
        if adapter.session.state() != &SessionState::Running {
            web_plugin_shutdown(&adapter.canvas_id);
            return Err("The web plugin protocol handshake failed.".to_owned());
        }
        Ok(adapter)
    }

    pub(super) fn send(&mut self, messages: Vec<Message>) -> Result<(), String> {
        for message in messages {
            match &message {
                Message::Screens(set) => {
                    self.session.enqueue_request(set.request_id, message, now())
                }
                _ => self.session.enqueue(message),
            }
            .map_err(queue_error)?;
        }
        self.flush()?;
        self.session.tick(now());
        match self.session.state() {
            SessionState::Running => Ok(()),
            state => Err(format!("Web plugin session stopped: {state:?}")),
        }
    }

    pub(super) fn take_client_messages(&mut self) -> Vec<TunnelMessage> {
        std::mem::take(&mut self.client_messages)
    }

    pub(super) fn take_editor_messages(&mut self) -> Vec<EditorMessage> {
        std::mem::take(&mut self.editor_messages)
    }

    pub(super) fn take_region_sizes(&mut self) -> Vec<RegionSize> {
        std::mem::take(&mut self.region_sizes)
    }

    pub(super) fn poll(&mut self) -> Result<(), String> {
        let responses = web_plugin_poll(&self.canvas_id).map_err(js_error)?;
        self.receive_all(&responses)
    }

    pub(super) fn render(&mut self) -> Result<Option<Duration>, String> {
        self.repaint = None;
        let responses = web_plugin_render(&self.canvas_id).map_err(js_error)?;
        self.receive_all(&responses)?;
        Ok(self.repaint.take())
    }

    pub(super) fn take_layout(&mut self) -> Option<ScreenLayout> {
        self.layout.take()
    }

    pub(super) fn shutdown(&mut self) {
        self.session.shutdown(now());
        let _ = self.flush();
        web_plugin_shutdown(&self.canvas_id);
    }

    fn flush(&mut self) -> Result<(), String> {
        loop {
            let frames = js_sys::Array::new();
            while let Some(message) = self.session.next_outbound() {
                let frame = encode_frame(&message).map_err(|error| error.to_string())?;
                frames.push(&js_sys::Uint8Array::from(frame.as_slice()).into());
            }
            if frames.length() == 0 {
                return Ok(());
            }
            let responses = web_plugin_send(&self.canvas_id, frames).map_err(js_error)?;
            self.receive_all(&responses)?;
        }
    }

    fn receive_all(&mut self, responses: &js_sys::Array) -> Result<(), String> {
        for response in responses.iter() {
            let response = js_sys::Uint8Array::new(&response).to_vec();
            match decode(&response)? {
                Message::Client(message) => self.client_messages.push(message),
                Message::Layout(layout) => self.layout = Some(layout),
                Message::RegionSizes(sizes) => self.region_sizes.extend(sizes),
                Message::FrameReady(frame) => {
                    self.repaint = frame.repaint_after_micros.map(Duration::from_micros);
                }
                Message::Editor(EditorMessage::Acknowledged { .. }) => {}
                Message::Editor(
                    message @ (EditorMessage::OpenBlock { .. }
                    | EditorMessage::DragAccepted { .. }
                    | EditorMessage::IntrinsicSize { .. }
                    | EditorMessage::AspectRatio { .. }
                    | EditorMessage::PickFile { .. }
                    | EditorMessage::CreationReady { .. }
                    | EditorMessage::CreationBlock { .. }),
                ) => {
                    self.editor_messages.push(message);
                }
                message => self.session.receive(message, now()),
            }
        }
        Ok(())
    }
}

fn decode(frame: &[u8]) -> Result<Message, String> {
    block_plugin_api::decode_frame(frame).map_err(|error| error.to_string())
}

fn queue_error(error: QueueError) -> String {
    format!("The web plugin input queue failed: {error:?}")
}

fn js_error(error: JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| "The web plugin JavaScript adapter failed.".to_owned())
}

fn now() -> u64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now() as u64)
        .unwrap_or_default()
}
