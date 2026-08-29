use std::time::Duration;

use block_plugin_api::{Message, PluginManifest, ScreenLayout};
use eframe::egui;

use super::backend::Backend;

const UNSUPPORTED: &str = "Plugins are not supported on this platform.";

pub(super) struct Unavailable;

impl Backend for Unavailable {
    type Frame = ();

    fn new(_plugin: &PluginManifest, _context: &egui::Context) -> Self {
        Self
    }

    fn start(&mut self, _plugin: &PluginManifest, _context: &egui::Context) {}

    fn send(&mut self, _messages: Vec<Message>) {}

    fn receive(&mut self) -> Vec<Message> {
        Vec::new()
    }

    fn frame(&mut self, _layout: &ScreenLayout, _pass: u64) -> Option<Self::Frame> {
        None
    }

    fn take_error(&mut self) -> Option<String> {
        Some(UNSUPPORTED.to_owned())
    }

    fn state(&self) -> &'static str {
        "unsupported"
    }

    fn uptime(&self) -> Option<Duration> {
        None
    }

    fn shutdown(&mut self) {}
}
