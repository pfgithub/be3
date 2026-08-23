use std::time::Duration;

use block_plugin_api::{Message, PluginManifest, ScreenLayout};
use eframe::egui;
use wasm_bindgen::JsCast;

mod adapter;
pub(super) mod renderer;

use super::runtime::{self, Backend, Update};
use adapter::WebProtocolAdapter;

const RENDERER_REQUIRED: &str = "wgpu is not available in this build.";
const REPAINT_SLACK_MILLISECONDS: f64 = 2.0;

pub(super) struct Web {
    plugin_id: String,
    canvas_id: String,
    url: String,
    adapter: Option<WebProtocolAdapter>,
    start: u64,
    starting: bool,
    started: f64,
    error: Option<String>,
    pending: Vec<Message>,
    dirty: bool,
    rendered_pass: u64,
    repaint_at: Option<f64>,
}

impl Backend for Web {
    type Frame = renderer::WebFrame;

    fn install(creation_context: &eframe::CreationContext<'_>) -> Result<(), String> {
        match renderer::install(creation_context) {
            true => Ok(()),
            false => Err(RENDERER_REQUIRED.to_owned()),
        }
    }

    fn new(plugin: &PluginManifest, _context: &egui::Context) -> Self {
        Self {
            plugin_id: plugin.identity.id.clone(),
            canvas_id: format!("plugin-canvas-{}", plugin.identity.id),
            url: plugin
                .entry_points
                .web
                .as_deref()
                .and_then(|entry| {
                    crate::editors::plugin::discovery::entry_point(&plugin.identity.id, entry)
                })
                .unwrap_or_default(),
            adapter: None,
            start: 0,
            starting: false,
            started: now(),
            error: None,
            pending: Vec::new(),
            dirty: false,
            rendered_pass: u64::MAX,
            repaint_at: None,
        }
    }

    fn start(&mut self, _plugin: &PluginManifest, context: &egui::Context) {
        if let Some(mut adapter) = self.adapter.take() {
            adapter.shutdown();
        }
        self.start += 1;
        self.starting = true;
        self.started = now();
        self.error = None;
        self.pending.clear();
        create_canvas(&self.canvas_id);
        let start = self.start;
        let plugin_id = self.plugin_id.clone();
        let canvas_id = self.canvas_id.clone();
        let url = self.url.clone();
        let context = context.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let mut result = Some(WebProtocolAdapter::start(url, canvas_id.clone()).await);
            runtime::with(&plugin_id, |runtime| {
                runtime.backend.attach(start, &mut result);
            });
            if let Some(result) = result {
                if let Ok(mut adapter) = result {
                    adapter.shutdown();
                }
                remove_canvas(&canvas_id);
            }
            context.request_repaint();
        });
    }

    fn send(&mut self, messages: Vec<Message>) {
        if self.adapter.is_none() {
            if self.starting {
                self.pending.extend(messages);
            }
            return;
        }
        self.dirty = true;
        if let Err(error) = self.adapter.as_mut().unwrap().send(messages) {
            self.error = Some(error);
        }
    }

    fn poll(&mut self, _context: &egui::Context) -> Update {
        let Some(adapter) = &mut self.adapter else {
            return Update::default();
        };
        if let Err(error) = adapter.poll() {
            self.error = Some(error);
            return Update::default();
        }
        self.take_output()
    }

    fn frame(&mut self, layout: &ScreenLayout, pass: u64) -> Self::Frame {
        renderer::WebFrame {
            size: [layout.width, layout.height],
            canvas_id: self.canvas_id.clone(),
            plugin_id: self.plugin_id.clone(),
            pass,
        }
    }

    fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    fn state(&self) -> &'static str {
        match (self.adapter.is_some(), self.starting) {
            (true, _) => "running",
            (false, true) => "starting",
            (false, false) => "stopped",
        }
    }

    fn uptime(&self) -> Option<Duration> {
        (self.adapter.is_some() || self.starting)
            .then(|| Duration::from_secs_f64((now() - self.started).max(0.0) / 1000.0))
    }

    fn shutdown(&mut self) {
        if let Some(mut adapter) = self.adapter.take() {
            adapter.shutdown();
        }
        if !self.starting {
            remove_canvas(&self.canvas_id);
        }
    }
}

impl Web {
    /// Takes the adapter a pending start produced, unless the runtime was
    /// restarted while it was loading. What is left in `result` belongs to
    /// nobody and is shut down by the caller.
    fn attach(&mut self, start: u64, result: &mut Option<Result<WebProtocolAdapter, String>>) {
        if start != self.start || !self.starting {
            return;
        }
        self.starting = false;
        match result.take() {
            Some(Ok(adapter)) => {
                self.adapter = Some(adapter);
                let pending = std::mem::take(&mut self.pending);
                self.send(pending);
            }
            Some(Err(error)) => {
                self.error = Some(error);
                remove_canvas(&self.canvas_id);
            }
            None => {}
        }
    }

    fn take_output(&mut self) -> Update {
        let Some(adapter) = &mut self.adapter else {
            return Update::default();
        };
        Update {
            layout: adapter.take_layout(),
            client: adapter.take_client_messages(),
            editor: adapter.take_editor_messages(),
            sizes: adapter.take_region_sizes(),
        }
    }

    /// Runs the plugin's own frame, once per host pass, when something it was
    /// told about changed or when it asked to be woken up again.
    fn draw(&mut self, pass: u64, context: &egui::Context) -> (bool, Update) {
        if self.rendered_pass == pass || self.adapter.is_none() {
            return (false, Update::default());
        }
        self.rendered_pass = pass;
        let due = match self.repaint_at {
            Some(deadline) if deadline <= now() + REPAINT_SLACK_MILLISECONDS => true,
            Some(deadline) => {
                context.request_repaint_after(Duration::from_secs_f64(
                    (deadline - now()).max(0.0) / 1000.0,
                ));
                false
            }
            None => false,
        };
        if !self.dirty && !due {
            return (false, Update::default());
        }
        self.dirty = false;
        self.repaint_at = None;
        match self.adapter.as_mut().unwrap().render() {
            Ok(Some(delay)) => {
                self.repaint_at = Some(now() + delay.as_secs_f64() * 1000.0);
                context.request_repaint_after(delay);
            }
            Ok(None) => {}
            Err(error) => {
                self.error = Some(error);
                context.request_repaint();
                return (false, Update::default());
            }
        }
        (true, self.take_output())
    }
}

/// Called while the presenter prepares its copy of the plugin's canvas, to
/// give the plugin the chance to draw the frame that is about to be copied.
pub(super) fn render(plugin_id: &str, pass: u64) -> bool {
    runtime::with(plugin_id, |runtime| {
        let context = runtime.context.clone();
        let (painted, update) = runtime.backend.draw(pass, &context);
        runtime.apply(update);
        painted
    })
    .unwrap_or_default()
}

fn now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now())
        .unwrap_or_default()
}

fn create_canvas(canvas_id: &str) {
    (|| -> Option<()> {
        let document = web_sys::window()?.document()?;
        if document.get_element_by_id(canvas_id).is_some() {
            return Some(());
        }
        let canvas: web_sys::HtmlCanvasElement =
            document.create_element("canvas").ok()?.dyn_into().ok()?;
        canvas.set_id(canvas_id);
        let _ = canvas.style().set_property("left", "-10000px");
        let _ = canvas.style().set_property("position", "fixed");
        let _ = canvas.style().set_property("top", "0");
        let _ = canvas.style().set_property("visibility", "hidden");
        document.body()?.append_child(&canvas).ok()?;
        Some(())
    })();
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
