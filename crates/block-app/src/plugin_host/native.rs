use std::{path::PathBuf, time::Instant};

use block_plugin_api::{Message, PluginManifest, ScreenLayout};
use eframe::egui;

use super::{
    platform::{self, Frame as PlatformFrame, RENDERER_REQUIRED},
    process::{Process, Received, SurfaceEvent},
    runtime::Backend,
};

pub(super) struct Native {
    process: Option<Process>,
    started: Instant,
    pending_frame: Option<PlatformFrame>,
    pending_layouts: Vec<ScreenLayout>,
    path: PathBuf,
}

impl Backend for Native {
    type Frame = PlatformFrame;

    fn install(creation_context: &eframe::CreationContext<'_>) -> Result<(), String> {
        match platform::install(creation_context) {
            true => Ok(()),
            false => Err(RENDERER_REQUIRED.to_owned()),
        }
    }

    fn new(plugin: &PluginManifest, _context: &egui::Context) -> Self {
        Self {
            process: None,
            started: Instant::now(),
            pending_frame: None,
            pending_layouts: Vec::new(),
            path: platform::entry_point(plugin),
        }
    }

    fn start(&mut self, _plugin: &PluginManifest, context: &egui::Context) {
        self.started = Instant::now();
        self.process = Some(Process::launch(self.path.clone(), context.clone()));
    }

    fn send(&mut self, messages: Vec<Message>) {
        if let Some(process) = &self.process {
            process.send(messages);
        }
    }

    fn receive(&mut self) -> Vec<Message> {
        let Some(process) = &self.process else {
            return Vec::new();
        };
        let mut messages = Vec::new();
        let mut frames: Vec<SurfaceEvent> = Vec::new();
        for received in process.receive() {
            match received {
                Received::Message(Message::Layout(layout)) => self.pending_layouts.push(layout),
                Received::Message(message) => messages.push(message),
                Received::Surface(event) => {
                    if let SurfaceEvent::Surface(descriptor, _) = &event {
                        if let Some(layout) = self.take_layout(descriptor.generation) {
                            messages.push(Message::Layout(layout));
                        }
                        frames.clear();
                    } else if matches!(frames.last(), Some(SurfaceEvent::Frame(_))) {
                        frames.pop();
                    }
                    frames.push(event);
                }
            }
        }
        if !frames.is_empty() {
            self.pending_frame = Some(PlatformFrame::Events(frames));
        }
        messages
    }

    fn frame(&mut self, _layout: &ScreenLayout, _pass: u64) -> Self::Frame {
        self.pending_frame
            .take()
            .unwrap_or(PlatformFrame::Events(Vec::new()))
    }

    fn take_error(&mut self) -> Option<String> {
        let error = self.process.as_ref().and_then(Process::take_exit)?;
        self.process = None;
        Some(format!("plugin process exited: {error}"))
    }

    fn state(&self) -> &'static str {
        match self.process.is_some() {
            true => "running",
            false => "stopped",
        }
    }

    fn uptime(&self) -> Option<std::time::Duration> {
        self.process.is_some().then(|| self.started.elapsed())
    }

    fn shutdown(&mut self) {
        if let Some(mut process) = self.process.take() {
            process.shutdown();
        }
    }
}

impl Native {
    /// The layout a newly arrived surface was drawn with. Layouts arrive
    /// ahead of the surfaces that use them, so the pending ones are held
    /// until the surface naming their generation shows up.
    fn take_layout(&mut self, generation: u64) -> Option<ScreenLayout> {
        let index = self
            .pending_layouts
            .iter()
            .position(|layout| layout.generation == generation)?;
        let layout = self.pending_layouts.remove(index);
        self.pending_layouts
            .retain(|pending| pending.generation > generation);
        eprintln!(
            "plugin host took layout generation {generation} with {} screens",
            layout.screens.len()
        );
        Some(layout)
    }
}
