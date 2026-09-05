use std::time::Duration;

use block_plugin_api::{Message, PluginManifest, ScreenLayout};
use eframe::egui;
use wasm_bindgen::JsCast;

mod adapter;
pub(super) mod renderer;

use super::backend::Backend;
use adapter::WebProtocolAdapter;

pub(super) struct Web {
    canvas_id: String,
    url: String,
    adapter: Option<WebProtocolAdapter>,
    started: f64,
    error: Option<String>,
}

impl Backend for Web {
    type Frame = renderer::WebFrame;

    fn new(plugin: &PluginManifest, _context: &egui::Context) -> Self {
        Self {
            canvas_id: format!("plugin-canvas-{}", plugin.identity.id),
            url: crate::editors::plugin::discovery::entry_point(
                &plugin.identity.id,
                &plugin.entry_point,
            )
            .unwrap_or_default(),
            adapter: None,
            started: now(),
            error: None,
        }
    }

    fn start(&mut self, _plugin: &PluginManifest, context: &egui::Context) {
        self.shutdown();
        self.started = now();
        self.error = None;
        let Some(canvas) = create_canvas(&self.canvas_id) else {
            self.error = Some("The plugin canvas could not be created.".to_owned());
            return;
        };
        match WebProtocolAdapter::start(&self.url, &canvas, context) {
            Ok(adapter) => self.adapter = Some(adapter),
            Err(error) => {
                self.error = Some(error);
                remove_canvas(&self.canvas_id);
            }
        }
    }

    fn send(&mut self, messages: Vec<Message>) {
        let Some(adapter) = &mut self.adapter else {
            return;
        };
        if let Err(error) = adapter.send(messages) {
            self.error = Some(error);
        }
    }

    fn receive(&mut self) -> Vec<Message> {
        let Some(adapter) = &mut self.adapter else {
            return Vec::new();
        };
        if let Err(error) = adapter.poll() {
            self.error = Some(error);
            return Vec::new();
        }
        adapter.take_received()
    }

    fn frame(&mut self, layout: &ScreenLayout, _pass: u64) -> Option<Self::Frame> {
        Some(renderer::WebFrame {
            size: [layout.width, layout.height],
            canvas_id: self.canvas_id.clone(),
            drawn: self.adapter.as_ref().map(WebProtocolAdapter::frames),
        })
    }

    fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    fn state(&self) -> &'static str {
        match &self.adapter {
            Some(adapter) if adapter.running() => "running",
            Some(_) => "starting",
            None => "stopped",
        }
    }

    fn uptime(&self) -> Option<Duration> {
        self.adapter
            .is_some()
            .then(|| Duration::from_secs_f64((now() - self.started).max(0.0) / 1000.0))
    }

    fn shutdown(&mut self) {
        if let Some(mut adapter) = self.adapter.take() {
            adapter.shutdown();
        }
        remove_canvas(&self.canvas_id);
    }
}

fn now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now())
        .unwrap_or_default()
}

fn create_canvas(canvas_id: &str) -> Option<web_sys::HtmlCanvasElement> {
    let document = web_sys::window()?.document()?;
    let canvas: web_sys::HtmlCanvasElement =
        document.create_element("canvas").ok()?.dyn_into().ok()?;
    canvas.set_id(canvas_id);
    let _ = canvas.style().set_property("left", "-10000px");
    let _ = canvas.style().set_property("position", "fixed");
    let _ = canvas.style().set_property("top", "0");
    let _ = canvas.style().set_property("visibility", "hidden");
    document.body()?.append_child(&canvas).ok()?;
    Some(canvas)
}

fn remove_canvas(canvas_id: &str) {
    let Some(canvas) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(canvas_id))
    else {
        return;
    };
    canvas.remove();
}
