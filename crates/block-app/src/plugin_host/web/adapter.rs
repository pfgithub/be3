use block_plugin_api::{
    encode_frame, Capability, DelegatedClientMessage, EditorMessage, HostSession, Message,
    QueueError, ScreenLayout, SessionState, SurfaceMechanism,
};
use wasm_bindgen::prelude::*;

const COUNTER_URL: &str = "/counter.js";

#[wasm_bindgen(inline_js = "
const plugins = new Map();

export async function web_plugin_start(url, canvasId) {
    const module = await import(url);
    await module.default();
    await module.start(canvasId);
    plugins.set(canvasId, module);
    return module.hello();
}

export function web_plugin_send(canvasId, frame) {
    const module = plugins.get(canvasId);
    if (!module) throw new Error('the web plugin is not running');
    return module.receive(frame);
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
    fn web_plugin_send(canvas_id: &str, frame: &[u8]) -> Result<js_sys::Array, JsValue>;
    fn web_plugin_shutdown(canvas_id: &str);
}

pub(super) struct WebProtocolAdapter {
    canvas_id: &'static str,
    session: HostSession,
    client_messages: Vec<DelegatedClientMessage>,
    layout: Option<ScreenLayout>,
}

impl WebProtocolAdapter {
    pub(super) async fn start(canvas_id: &'static str) -> Result<Self, String> {
        let hello = web_plugin_start(COUNTER_URL, canvas_id)
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
            layout: None,
        };
        adapter.flush()?;
        if adapter.session.state() != &SessionState::Running {
            web_plugin_shutdown(canvas_id);
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

    pub(super) fn take_client_messages(&mut self) -> Vec<DelegatedClientMessage> {
        std::mem::take(&mut self.client_messages)
    }

    pub(super) fn take_layout(&mut self) -> Option<ScreenLayout> {
        self.layout.take()
    }

    pub(super) fn shutdown(&mut self) {
        self.session.shutdown(now());
        let _ = self.flush();
        web_plugin_shutdown(self.canvas_id);
    }

    fn flush(&mut self) -> Result<(), String> {
        while let Some(message) = self.session.next_outbound() {
            let frame = encode_frame(&message).map_err(|error| error.to_string())?;
            let responses = web_plugin_send(self.canvas_id, &frame).map_err(js_error)?;
            for response in responses.iter() {
                let response = js_sys::Uint8Array::new(&response).to_vec();
                match decode(&response)? {
                    Message::Client(message) => self.client_messages.push(message),
                    Message::Layout(layout) => self.layout = Some(layout),
                    Message::Editor(EditorMessage::Acknowledged { .. }) => {}
                    message => self.session.receive(message, now()),
                }
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
