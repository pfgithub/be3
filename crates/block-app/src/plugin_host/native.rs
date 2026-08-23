use std::{path::PathBuf, time::Instant};

use block_plugin_api::{Message, PluginManifest, ScreenLayout};
use eframe::egui;

#[cfg(target_os = "linux")]
use super::linux::{install as install_presenter, LinuxFrame as PlatformFrame, RENDERER_REQUIRED};
#[cfg(target_os = "windows")]
use super::windows::{
    install as install_presenter, WindowsFrame as PlatformFrame, RENDERER_REQUIRED,
};
use super::{
    process::{Process, SurfaceEvent},
    runtime::{Backend, Update},
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
        match install_presenter(creation_context) {
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
            path: plugin_path(plugin),
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

    fn poll(&mut self, _context: &egui::Context) -> Update {
        let (client, editor, sizes, frames, layouts) = {
            let Some(process) = &self.process else {
                return Update::default();
            };
            (
                process.client_messages(),
                process.editor_messages(),
                process.region_sizes(),
                process.latest_surface(),
                process.layouts(),
            )
        };
        self.pending_layouts.extend(layouts);
        let layout = self.take_layout(&frames);
        if !frames.is_empty() {
            self.pending_frame = Some(PlatformFrame::Events(frames));
        }
        Update {
            layout,
            client,
            editor,
            sizes,
        }
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
    fn take_layout(&mut self, frames: &[SurfaceEvent]) -> Option<ScreenLayout> {
        let mut taken = None;
        for event in frames {
            let SurfaceEvent::Surface(descriptor, _) = event else {
                continue;
            };
            let Some(index) = self
                .pending_layouts
                .iter()
                .position(|layout| layout.generation == descriptor.generation)
            else {
                continue;
            };
            let layout = self.pending_layouts.remove(index);
            self.pending_layouts
                .retain(|pending| pending.generation > layout.generation);
            taken = Some(layout);
        }
        taken
    }
}

fn plugin_path(plugin: &PluginManifest) -> PathBuf {
    #[cfg(target_os = "windows")]
    let entry = plugin.entry_points.windows.as_deref().unwrap_or_default();
    #[cfg(target_os = "linux")]
    let entry = plugin.entry_points.linux.as_deref().unwrap_or_default();
    crate::editors::plugin::discovery::entry_point(&plugin.identity.id, entry).unwrap_or_default()
}
