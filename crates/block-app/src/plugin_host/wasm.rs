use std::{cell::RefCell, path::PathBuf, time::Instant};

use block_plugin_api::{decode_frame, encode_frame, Message, PluginManifest, ScreenLayout};
use block_wasm_host::Plugin;
use eframe::{
    egui,
    egui_wgpu::{self, wgpu},
};

mod surface;

pub(super) use surface::{presenter, Presenter, WasmFrame};

const SCREENS_SURFACE: u32 = 0;
const NO_ENTRY_POINT: &str = "This plugin has no wasm entry point.";
const NO_GPU: &str = "The plugin host has no graphics device.";

thread_local! {
    static GPU: RefCell<Option<(wgpu::Device, wgpu::Queue)>> = const { RefCell::new(None) };
}

fn remember_gpu(render_state: &egui_wgpu::RenderState) {
    GPU.with(|gpu| {
        *gpu.borrow_mut() = Some((render_state.device.clone(), render_state.queue.clone()));
    });
}

fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    GPU.with(|gpu| gpu.borrow().clone())
}

pub(super) struct Wasm {
    plugin: Option<Plugin>,
    module: Option<PathBuf>,
    started: Instant,
    error: Option<String>,
}

impl super::backend::Backend for Wasm {
    type Frame = WasmFrame;

    fn new(plugin: &PluginManifest, _context: &egui::Context) -> Self {
        Self {
            plugin: None,
            module: plugin.entry_points.wasm.as_deref().and_then(|entry| {
                crate::editors::plugin::discovery::entry_point(&plugin.identity.id, entry)
            }),
            started: Instant::now(),
            error: None,
        }
    }

    fn start(&mut self, _plugin: &PluginManifest, _context: &egui::Context) {
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
        let mut plugin = match Plugin::from_file(&module, device, queue) {
            Ok(plugin) => plugin,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        if let Err(error) = plugin.start() {
            self.error = Some(error);
            return;
        }
        self.plugin = Some(plugin);
    }

    fn send(&mut self, messages: Vec<Message>) {
        let Some(plugin) = &mut self.plugin else {
            return;
        };
        let mut failure = None;
        for message in messages {
            match encode_frame(&message) {
                Ok(frame) => plugin.send(frame),
                Err(error) => failure = Some(error.to_string()),
            }
        }
        if let Some(failure) = failure {
            self.error.get_or_insert(failure);
        }
    }

    fn receive(&mut self) -> Vec<Message> {
        let Some(plugin) = &mut self.plugin else {
            return Vec::new();
        };
        if let Err(error) = plugin.step() {
            self.error = Some(error);
            self.shutdown();
            return Vec::new();
        }
        let mut messages = Vec::new();
        let mut failure = None;
        for frame in plugin.take_outbound() {
            match decode_frame(&frame) {
                Ok(message) => messages.push(message),
                Err(error) => failure = Some(format!("{error:?}")),
            }
        }
        if let Some(failure) = failure {
            self.error.get_or_insert(failure);
        }
        messages
    }

    fn frame(&mut self, _layout: &ScreenLayout, _pass: u64) -> Option<WasmFrame> {
        let plugin = self.plugin.as_mut()?;
        let presented = plugin.take_presented();
        if !presented.contains(&SCREENS_SURFACE) {
            return None;
        }
        let (texture, generation) = plugin.surface(SCREENS_SURFACE)?;
        Some(WasmFrame {
            texture,
            generation,
        })
    }

    fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    fn state(&self) -> &'static str {
        match self.plugin.is_some() {
            true => "running",
            false => "stopped",
        }
    }

    fn uptime(&self) -> Option<std::time::Duration> {
        self.plugin.is_some().then(|| self.started.elapsed())
    }

    fn shutdown(&mut self) {
        if let Some(mut plugin) = self.plugin.take() {
            plugin.stop();
        }
    }
}
