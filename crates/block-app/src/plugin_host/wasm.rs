use std::{
    cell::RefCell,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
    time::Instant,
};

use crate::editors::plugin::discovery::{self, Module};

use block_plugin_api::{decode_frame, encode_frame, Message, PluginManifest, ScreenLayout};
use block_wasm_host::{Host, Plugin};
use eframe::{
    egui,
    egui_wgpu::{self, wgpu},
};

mod surface;

pub(super) use surface::{presenter, Presenter, WasmFrame};

const SCREENS_SURFACE: u32 = 0;
const NO_ENTRY_POINT: &str = "This plugin has no wasm entry point.";
const NO_GPU: &str = "The plugin host has no graphics device.";
const STOPPED: &str = "The plugin worker stopped.";

thread_local! {
    static GPU: RefCell<Option<(wgpu::Device, wgpu::Queue)>> = const { RefCell::new(None) };
    static CACHE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub(crate) fn cache_in(directory: PathBuf) {
    CACHE.with(|cache| *cache.borrow_mut() = Some(directory));
}

fn remember_gpu(render_state: &egui_wgpu::RenderState) {
    GPU.with(|gpu| {
        *gpu.borrow_mut() = Some((render_state.device.clone(), render_state.queue.clone()));
    });
}

fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    GPU.with(|gpu| gpu.borrow().clone())
}

fn cache() -> Option<PathBuf> {
    CACHE.with(|cache| cache.borrow().clone())
}

struct Target {
    texture: wgpu::Texture,
    generation: u64,
}

struct Produced {
    outbound: Vec<Vec<u8>>,
    presented: Option<Target>,
}

impl Produced {
    fn is_empty(&self) -> bool {
        self.outbound.is_empty() && self.presented.is_none()
    }
}

enum Command {
    Step(Vec<Vec<u8>>),
}

enum Event {
    Ready(Produced),
    Stepped(Produced),
    Failed(String),
}

struct Worker {
    commands: Sender<Command>,
    events: Receiver<Event>,
    pending: Vec<Vec<u8>>,
    ready: bool,
    stepping: bool,
    target: Option<Target>,
    presented: bool,
}

pub(super) struct Wasm {
    worker: Option<Worker>,
    module: Option<Module>,
    started: Instant,
    error: Option<String>,
}

impl super::backend::Backend for Wasm {
    type Frame = WasmFrame;

    fn new(plugin: &PluginManifest, _context: &egui::Context) -> Self {
        Self {
            worker: None,
            module: plugin
                .entry_points
                .wasm
                .as_deref()
                .and_then(|entry| discovery::module(&plugin.identity.id, entry)),
            started: Instant::now(),
            error: None,
        }
    }

    fn start(&mut self, plugin: &PluginManifest, context: &egui::Context) {
        self.shutdown();
        self.started = Instant::now();
        self.error = None;
        let Some(module) = self.module.clone() else {
            self.error = Some(NO_ENTRY_POINT.to_owned());
            return;
        };
        let Some((device, queue)) = gpu() else {
            self.error = Some(NO_GPU.to_owned());
            return;
        };
        let host = match Host::new(device, queue, cache().as_deref()) {
            Ok(host) => host,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let (commands, orders) = mpsc::channel();
        let (reports, events) = mpsc::channel();
        let context = context.clone();
        let spawned = thread::Builder::new()
            .name(format!("plugin {}", plugin.identity.id))
            .spawn(move || run(host, module, orders, reports, context));
        match spawned {
            Ok(_) => {
                self.worker = Some(Worker {
                    commands,
                    events,
                    pending: Vec::new(),
                    ready: false,
                    stepping: false,
                    target: None,
                    presented: false,
                })
            }
            Err(error) => self.error = Some(format!("the plugin worker could not start: {error}")),
        }
    }

    fn send(&mut self, messages: Vec<Message>) {
        let Some(worker) = &mut self.worker else {
            return;
        };
        let mut failure = None;
        for message in messages {
            match encode_frame(&message) {
                Ok(frame) => worker.pending.push(frame),
                Err(error) => failure = Some(error.to_string()),
            }
        }
        if let Some(failure) = failure {
            self.error.get_or_insert(failure);
        }
    }

    fn receive(&mut self) -> Vec<Message> {
        let Some(worker) = &mut self.worker else {
            return Vec::new();
        };
        let mut frames = Vec::new();
        let mut failure = None;
        loop {
            match worker.events.try_recv() {
                Ok(Event::Ready(produced)) => {
                    worker.ready = true;
                    worker.absorb(produced, &mut frames);
                }
                Ok(Event::Stepped(produced)) => {
                    worker.stepping = false;
                    worker.absorb(produced, &mut frames);
                }
                Ok(Event::Failed(error)) => {
                    failure = Some(error);
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    failure = Some(STOPPED.to_owned());
                    break;
                }
            }
        }
        if failure.is_none() && worker.ready && !worker.stepping {
            let step = Command::Step(std::mem::take(&mut worker.pending));
            match worker.commands.send(step) {
                Ok(()) => worker.stepping = true,
                Err(_) => failure = Some(STOPPED.to_owned()),
            }
        }
        if let Some(failure) = failure {
            self.error.get_or_insert(failure);
            self.worker = None;
        }
        decode(frames, &mut self.error)
    }

    fn frame(&mut self, _layout: &ScreenLayout, _pass: u64) -> Option<WasmFrame> {
        let worker = self.worker.as_mut()?;
        if !std::mem::take(&mut worker.presented) {
            return None;
        }
        let target = worker.target.as_ref()?;
        Some(WasmFrame {
            texture: target.texture.clone(),
            generation: target.generation,
        })
    }

    fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    fn state(&self) -> &'static str {
        match &self.worker {
            Some(worker) if worker.ready => "running",
            Some(_) => "starting",
            None => "stopped",
        }
    }

    fn uptime(&self) -> Option<std::time::Duration> {
        self.worker.is_some().then(|| self.started.elapsed())
    }

    fn shutdown(&mut self) {
        self.worker = None;
    }
}

impl Worker {
    fn absorb(&mut self, produced: Produced, frames: &mut Vec<Vec<u8>>) {
        frames.extend(produced.outbound);
        if let Some(target) = produced.presented {
            self.target = Some(target);
            self.presented = true;
        }
    }
}

fn decode(frames: Vec<Vec<u8>>, error: &mut Option<String>) -> Vec<Message> {
    let mut messages = Vec::with_capacity(frames.len());
    for frame in frames {
        match decode_frame(&frame) {
            Ok(message) => messages.push(message),
            Err(failure) => {
                error.get_or_insert(format!("{failure:?}"));
            }
        }
    }
    messages
}

fn open(host: &Host, module: &Module) -> Result<Plugin, String> {
    #[cfg(not(target_os = "android"))]
    {
        host.load(module)
    }
    #[cfg(target_os = "android")]
    {
        host.load_bytes(module)
    }
}

fn run(
    host: Host,
    module: Module,
    orders: Receiver<Command>,
    reports: Sender<Event>,
    context: egui::Context,
) {
    let mut plugin = match open(&host, &module).and_then(|mut plugin| {
        plugin.start()?;
        Ok(plugin)
    }) {
        Ok(plugin) => plugin,
        Err(error) => {
            let _ = reports.send(Event::Failed(error));
            context.request_repaint();
            return;
        }
    };
    if !report(&reports, &context, Event::Ready(produced(&mut plugin))) {
        return;
    }
    for order in orders {
        let Command::Step(frames) = order;
        for frame in frames {
            plugin.send(frame);
        }
        let event = match plugin.step() {
            Ok(()) => Event::Stepped(produced(&mut plugin)),
            Err(error) => Event::Failed(error),
        };
        let failed = matches!(event, Event::Failed(_));
        if !report(&reports, &context, event) || failed {
            return;
        }
    }
    plugin.stop();
}

fn report(reports: &Sender<Event>, context: &egui::Context, event: Event) -> bool {
    let quiet = matches!(&event, Event::Stepped(produced) if produced.is_empty());
    let sent = reports.send(event).is_ok();
    if !quiet {
        context.request_repaint();
    }
    sent
}

fn produced(plugin: &mut Plugin) -> Produced {
    let outbound = plugin.take_outbound();
    let presented = plugin
        .take_presented()
        .contains(&SCREENS_SURFACE)
        .then(|| plugin.surface(SCREENS_SURFACE))
        .flatten()
        .map(|(texture, generation)| Target {
            texture,
            generation,
        });
    Produced {
        outbound,
        presented,
    }
}
