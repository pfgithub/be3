use block_plugin_api::{
    Capability, Hello, HostSession, InputEvent, Message, Modifiers, PhysicalKey, PluginIdentity,
    PointerButton, QueueError, SessionState, SurfaceMechanism, WheelUnit, PROTOCOL_VERSION,
};
use wasm_bindgen::prelude::*;

const PLUGIN_DEMO_URL: &str = "/plugin_demo.js";

#[wasm_bindgen(inline_js = "
const plugins = new Map();

export async function web_plugin_start(url, canvasId) {
    const module = await import(url);
    await module.default();
    await module.start(canvasId);
    plugins.set(canvasId, module);
}

export function web_plugin_resize(canvasId, width, height) {
    const canvas = document.getElementById(canvasId);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
}

function pointerEvent(canvasId, kind, x, y, button, buttons, alt, control, shift, command) {
    const canvas = document.getElementById(canvasId);
    const rect = canvas.getBoundingClientRect();
    const event = new PointerEvent(kind, {
        bubbles: true,
        clientX: rect.left + x,
        clientY: rect.top + y,
        button,
        buttons,
        altKey: alt,
        ctrlKey: control,
        shiftKey: shift,
        metaKey: command,
        pointerId: 1,
        pointerType: 'mouse',
    });
    queueMicrotask(() => (kind === 'pointerdown' ? canvas : document).dispatchEvent(event));
}

export function web_plugin_pointer(canvasId, kind, x, y, button, buttons, alt, control, shift, command) {
    pointerEvent(canvasId, kind, x, y, button, buttons, alt, control, shift, command);
}

export function web_plugin_wheel(canvasId, x, y, unit, alt, control, shift, command) {
    const canvas = document.getElementById(canvasId);
    const event = new WheelEvent('wheel', {
        bubbles: true,
        deltaX: -x * unit,
        deltaY: -y * unit,
        deltaMode: 0,
        altKey: alt,
        ctrlKey: control,
        shiftKey: shift,
        metaKey: command,
    });
    queueMicrotask(() => canvas.dispatchEvent(event));
}

export function web_plugin_key(canvasId, logical, pressed, repeat, alt, control, shift, command) {
    const canvas = document.getElementById(canvasId);
    const event = new KeyboardEvent(pressed ? 'keydown' : 'keyup', {
        bubbles: true,
        key: logical,
        repeat,
        altKey: alt,
        ctrlKey: control,
        shiftKey: shift,
        metaKey: command,
    });
    queueMicrotask(() => canvas.dispatchEvent(event));
}

export function web_plugin_text(canvasId, value) {
    const canvas = document.getElementById(canvasId);
    const event = new InputEvent('input', { bubbles: true, data: value, inputType: 'insertText' });
    queueMicrotask(() => canvas.dispatchEvent(event));
}

export function web_plugin_focus(canvasId, focused) {
    const canvas = document.getElementById(canvasId);
    queueMicrotask(() => focused ? canvas.focus() : canvas.blur());
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
    async fn web_plugin_start(url: &str, canvas_id: &str) -> Result<(), JsValue>;
    fn web_plugin_resize(canvas_id: &str, width: f32, height: f32);
    fn web_plugin_pointer(
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
    fn web_plugin_wheel(
        canvas_id: &str,
        x: f32,
        y: f32,
        unit: f32,
        alt: bool,
        control: bool,
        shift: bool,
        command: bool,
    );
    fn web_plugin_key(
        canvas_id: &str,
        logical: &str,
        pressed: bool,
        repeat: bool,
        alt: bool,
        control: bool,
        shift: bool,
        command: bool,
    );
    fn web_plugin_text(canvas_id: &str, value: &str);
    fn web_plugin_focus(canvas_id: &str, focused: bool);
    fn web_plugin_shutdown(canvas_id: &str);
}

pub(super) struct WebProtocolAdapter {
    canvas_id: &'static str,
    session: HostSession,
    modifiers: Modifiers,
    buttons: u16,
}

impl WebProtocolAdapter {
    pub(super) async fn start(canvas_id: &'static str) -> Result<Self, String> {
        web_plugin_start(PLUGIN_DEMO_URL, canvas_id)
            .await
            .map_err(js_error)?;
        let mut session = HostSession::new(
            "BE3 web host",
            vec![
                Capability::Input,
                Capability::Lifecycle,
                Capability::Surface(SurfaceMechanism::WebExternalImage),
            ],
        );
        session.start(now());
        session.receive(
            Message::Hello(Hello {
                minimum_version: PROTOCOL_VERSION,
                maximum_version: PROTOCOL_VERSION,
                plugin: PluginIdentity {
                    id: "plugin-demo".to_owned(),
                    name: "Plugin Demo".to_owned(),
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                },
                capabilities: vec![
                    Capability::Input,
                    Capability::Lifecycle,
                    Capability::Surface(SurfaceMechanism::WebExternalImage),
                ],
            }),
            now(),
        );
        let _ = session.next_outbound();
        if session.state() != &SessionState::Running {
            web_plugin_shutdown(canvas_id);
            return Err("The web plugin protocol handshake failed.".to_owned());
        }
        Ok(Self {
            canvas_id,
            session,
            modifiers: Modifiers::default(),
            buttons: 0,
        })
    }

    pub(super) fn send(&mut self, messages: Vec<Message>) -> Result<(), String> {
        for message in messages {
            match &message {
                Message::CreateViewport(viewport) => {
                    self.session
                        .enqueue_request(viewport.request_id, message, now())
                }
                _ => self.session.enqueue(message),
            }
            .map_err(queue_error)?;
        }
        while let Some(message) = self.session.next_outbound() {
            let acknowledgement = match &message {
                Message::CreateViewport(viewport) => Some(viewport.request_id),
                _ => None,
            };
            self.dispatch(message)?;
            if let Some(request_id) = acknowledgement {
                self.session
                    .receive(Message::Acknowledged { request_id }, now());
            }
        }
        self.session.tick(now());
        match self.session.state() {
            SessionState::Running => Ok(()),
            state => Err(format!("Web plugin session stopped: {state:?}")),
        }
    }

    pub(super) fn shutdown(&mut self) {
        self.session.shutdown(now());
        if matches!(self.session.next_outbound(), Some(Message::Shutdown)) {
            web_plugin_shutdown(self.canvas_id);
            self.session.receive(Message::ShutdownAcknowledged, now());
        }
    }

    fn dispatch(&mut self, message: Message) -> Result<(), String> {
        match message {
            Message::CreateViewport(viewport) => self.resize(viewport.metrics),
            Message::ResizeViewport(metrics) => self.resize(metrics),
            Message::Input(input) => {
                for event in input.events {
                    self.input(event);
                }
            }
            _ => return Err("The web adapter received an unsupported host message.".to_owned()),
        }
        Ok(())
    }

    fn resize(&self, metrics: block_plugin_api::ViewportMetrics) {
        web_plugin_resize(
            self.canvas_id,
            metrics.logical_width,
            metrics.logical_height,
        );
    }

    fn input(&mut self, event: InputEvent) {
        match event {
            InputEvent::PointerMoved { x, y } => self.pointer("pointermove", x, y, -1),
            InputEvent::PointerButton {
                button,
                pressed,
                x,
                y,
            } => {
                let index = pointer_button_index(button);
                let mask = 1 << index;
                if pressed {
                    self.buttons |= mask;
                } else {
                    self.buttons &= !mask;
                }
                self.pointer(
                    if pressed { "pointerdown" } else { "pointerup" },
                    x,
                    y,
                    index,
                );
            }
            InputEvent::Wheel { x, y, unit } => web_plugin_wheel(
                self.canvas_id,
                x,
                y,
                wheel_scale(unit),
                self.modifiers.alt,
                self.modifiers.control,
                self.modifiers.shift,
                self.modifiers.command,
            ),
            InputEvent::Key {
                physical,
                logical,
                pressed,
                repeat,
            } => {
                let _ = match physical {
                    PhysicalKey::Code(code) => Some(code),
                    PhysicalKey::Unidentified => None,
                };
                web_plugin_key(
                    self.canvas_id,
                    &logical,
                    pressed,
                    repeat,
                    self.modifiers.alt,
                    self.modifiers.control,
                    self.modifiers.shift,
                    self.modifiers.command,
                );
            }
            InputEvent::Text(text) => web_plugin_text(self.canvas_id, &text),
            InputEvent::Modifiers(modifiers) => self.modifiers = modifiers,
            InputEvent::Focus(focused) => web_plugin_focus(self.canvas_id, focused),
        }
    }

    fn pointer(&self, kind: &str, x: f32, y: f32, button: i16) {
        web_plugin_pointer(
            self.canvas_id,
            kind,
            x,
            y,
            button,
            self.buttons,
            self.modifiers.alt,
            self.modifiers.control,
            self.modifiers.shift,
            self.modifiers.command,
        );
    }
}

fn pointer_button_index(button: PointerButton) -> i16 {
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

fn now() -> u64 {
    js_sys::Date::now() as u64
}

fn queue_error(error: QueueError) -> String {
    format!("The web plugin queue rejected a message: {error:?}")
}

fn js_error(error: JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}
