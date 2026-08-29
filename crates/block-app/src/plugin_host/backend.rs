use std::time::Duration;

use block_plugin_api::{Message, PluginManifest, ScreenLayout};
use eframe::egui;

#[cfg(not(target_arch = "wasm32"))]
use super::wasm::Wasm;
#[cfg(target_arch = "wasm32")]
use super::web::Web;

pub(super) const NOT_INSTALLED: &str = "The plugin host is not installed.";

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

#[cfg(not(target_arch = "wasm32"))]
pub(super) type Platform = Wasm;
#[cfg(target_arch = "wasm32")]
pub(super) type Platform = Web;

pub(super) type Frame = <Platform as Backend>::Frame;

pub(super) struct Availability(pub(super) Result<(), String>);

impl Availability {
    pub(super) fn missing() -> Self {
        Self(Err(NOT_INSTALLED.to_owned()))
    }
}
