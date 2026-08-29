use std::time::Duration;

use block_plugin_api::{Message, PluginManifest, ScreenLayout};
use eframe::egui;

#[cfg(any(target_os = "windows", target_os = "linux"))]
use super::native::Native;
#[cfg(not(target_arch = "wasm32"))]
use super::wasm::Wasm;
#[cfg(target_arch = "wasm32")]
use super::web::Web;

pub(super) const NOT_INSTALLED: &str = "The plugin host is not installed.";
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub(super) const ONLY_HOSTED: &str = "This platform runs only plugins with a wasm entry point.";

pub(super) trait Backend: Sized {
    type Frame;

    fn new(plugin: &PluginManifest, context: &egui::Context) -> Self;

    fn start(&mut self, plugin: &PluginManifest, context: &egui::Context);

    fn send(&mut self, messages: Vec<Message>);

    fn receive(&mut self) -> Vec<Message>;

    fn frame(&mut self, layout: &ScreenLayout, pass: u64) -> Option<Self::Frame>;

    fn take_error(&mut self) -> Option<String>;

    fn state(&self) -> &'static str;

    fn uptime(&self) -> Option<Duration>;

    fn shutdown(&mut self);
}

pub(super) enum Platform {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    Process(Native),
    #[cfg(not(target_arch = "wasm32"))]
    Hosted(Wasm),
    #[cfg(target_arch = "wasm32")]
    Web(Web),
}

pub(super) enum Frame {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    Process(<Native as Backend>::Frame),
    #[cfg(not(target_arch = "wasm32"))]
    Hosted(<Wasm as Backend>::Frame),
    #[cfg(target_arch = "wasm32")]
    Web(<Web as Backend>::Frame),
}

macro_rules! dispatch {
    ($platform:expr, |$backend:ident| $body:expr) => {
        match $platform {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            Platform::Process($backend) => $body,
            #[cfg(not(target_arch = "wasm32"))]
            Platform::Hosted($backend) => $body,
            #[cfg(target_arch = "wasm32")]
            Platform::Web($backend) => $body,
        }
    };
}

impl Backend for Platform {
    type Frame = Frame;

    fn new(plugin: &PluginManifest, context: &egui::Context) -> Self {
        create(plugin, context)
    }

    fn start(&mut self, plugin: &PluginManifest, context: &egui::Context) {
        dispatch!(self, |backend| backend.start(plugin, context))
    }

    fn send(&mut self, messages: Vec<Message>) {
        dispatch!(self, |backend| backend.send(messages))
    }

    fn receive(&mut self) -> Vec<Message> {
        dispatch!(self, |backend| backend.receive())
    }

    fn frame(&mut self, layout: &ScreenLayout, pass: u64) -> Option<Frame> {
        match self {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            Platform::Process(backend) => backend.frame(layout, pass).map(Frame::Process),
            #[cfg(not(target_arch = "wasm32"))]
            Platform::Hosted(backend) => backend.frame(layout, pass).map(Frame::Hosted),
            #[cfg(target_arch = "wasm32")]
            Platform::Web(backend) => backend.frame(layout, pass).map(Frame::Web),
        }
    }

    fn take_error(&mut self) -> Option<String> {
        dispatch!(self, |backend| backend.take_error())
    }

    fn state(&self) -> &'static str {
        dispatch!(self, |backend| backend.state())
    }

    fn uptime(&self) -> Option<Duration> {
        dispatch!(self, |backend| backend.uptime())
    }

    fn shutdown(&mut self) {
        dispatch!(self, |backend| backend.shutdown())
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn create(plugin: &PluginManifest, context: &egui::Context) -> Platform {
    match hosted(plugin) {
        true => Platform::Hosted(Wasm::new(plugin, context)),
        false => Platform::Process(Native::new(plugin, context)),
    }
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "windows"),
    not(target_os = "linux")
))]
fn create(plugin: &PluginManifest, context: &egui::Context) -> Platform {
    Platform::Hosted(Wasm::new(plugin, context))
}

#[cfg(target_arch = "wasm32")]
fn create(plugin: &PluginManifest, context: &egui::Context) -> Platform {
    Platform::Web(Web::new(plugin, context))
}

pub(super) fn hosted(plugin: &PluginManifest) -> bool {
    plugin.entry_points.wasm.is_some()
}

pub(super) struct Availability {
    pub(super) platform: Result<(), String>,
    pub(super) hosted: Result<(), String>,
}

impl Availability {
    pub(super) fn missing() -> Self {
        Self {
            platform: Err(NOT_INSTALLED.to_owned()),
            hosted: Err(NOT_INSTALLED.to_owned()),
        }
    }

    pub(super) fn of(&self, plugin: &PluginManifest) -> &Result<(), String> {
        match hosted(plugin) {
            true => &self.hosted,
            false => &self.platform,
        }
    }
}
