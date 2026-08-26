use std::time::Duration;

use block_plugin_api::{Message, PluginManifest, ScreenLayout};
use eframe::{egui, egui_wgpu::wgpu};

use super::{
    presenter::{Regions, SurfacePresenter},
    runtime::Backend,
};

const UNSUPPORTED: &str = "Plugins are not supported on this platform.";

pub(super) struct Unavailable;

impl Backend for Unavailable {
    type Frame = ();

    fn install(_creation_context: &eframe::CreationContext<'_>) -> Result<(), String> {
        Err(UNSUPPORTED.to_owned())
    }

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

pub(super) enum UnavailablePresenter {}

impl SurfacePresenter for UnavailablePresenter {
    type Frame = ();

    fn replace(
        &mut self,
        _device: &wgpu::Device,
        _surface: u32,
        _frame: &Self::Frame,
    ) -> Result<(), String> {
        match *self {}
    }

    fn prepare(
        &mut self,
        _queue: &wgpu::Queue,
        _surface: u32,
        _frame: &Self::Frame,
    ) -> Result<(), String> {
        match *self {}
    }

    fn regions(&self) -> &Regions {
        match *self {}
    }

    fn paint(&self, _render_pass: &mut wgpu::RenderPass<'static>, _surface: u32, _slot: u32) {
        match *self {}
    }

    fn release(&mut self, _surface: u32) {
        match *self {}
    }
}
