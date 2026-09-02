use std::sync::mpsc::{self, Receiver, Sender};

use block_plugin_api::{WebViewCommand, WebViewEvent};
use eframe::egui;

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
mod native;
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
mod unsupported;

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use native::WebView;
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
use unsupported::WebView;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Bounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

pub(super) struct WebViewHost {
    view: Option<WebView>,
    events: Receiver<WebViewEvent>,
    sender: Sender<WebViewEvent>,
    pending: Vec<WebViewCommand>,
    bounds: Option<Bounds>,
    visible: bool,
    address: Option<String>,
    failed: bool,
}

impl Default for WebViewHost {
    fn default() -> Self {
        let (sender, events) = mpsc::channel();
        Self {
            view: None,
            events,
            sender,
            pending: Vec::new(),
            bounds: None,
            visible: false,
            address: None,
            failed: false,
        }
    }
}

impl WebViewHost {
    pub(super) fn command(&mut self, command: WebViewCommand) {
        self.pending.push(command);
    }

    pub(super) fn drive(
        &mut self,
        frame: &eframe::Frame,
        context: &egui::Context,
        rect: Option<egui::Rect>,
        events: &mut Vec<WebViewEvent>,
    ) {
        for command in std::mem::take(&mut self.pending) {
            self.apply(frame, command, events);
        }
        match rect.filter(egui::Rect::is_positive) {
            Some(rect) => {
                self.set_bounds(context, rect, events);
                self.set_visible(true, events);
            }
            None => self.set_visible(false, events),
        }
        while let Ok(event) = self.events.try_recv() {
            events.push(event);
        }
        let address = self.view.as_ref().and_then(WebView::url);
        if let Some(address) = address {
            if self.address.as_deref() != Some(address.as_str()) {
                self.address = Some(address.clone());
                events.push(WebViewEvent::Address(address));
            }
        }
    }

    fn apply(
        &mut self,
        frame: &eframe::Frame,
        command: WebViewCommand,
        events: &mut Vec<WebViewEvent>,
    ) {
        match command {
            WebViewCommand::Open(url) => self.open(frame, &url, events),
            WebViewCommand::Load(url) => {
                if let Some(view) = &self.view {
                    report(view.load_url(&url), events);
                }
            }
            WebViewCommand::Reload => {
                if let Some(view) = &self.view {
                    report(view.reload(), events);
                }
            }
            WebViewCommand::FocusApp => {
                if let Some(view) = &self.view {
                    report(view.focus_parent(), events);
                }
            }
            WebViewCommand::Close => self.close(events),
        }
    }

    fn open(&mut self, frame: &eframe::Frame, url: &str, events: &mut Vec<WebViewEvent>) {
        if self.view.is_some() || self.failed {
            return;
        }
        match WebView::new(frame, url, &self.sender) {
            Ok(view) => {
                self.view = Some(view);
                self.address = Some(url.to_owned());
                self.bounds = None;
                self.visible = false;
            }
            Err(error) => {
                self.failed = true;
                events.push(WebViewEvent::Failed(error));
            }
        }
    }

    pub(super) fn close(&mut self, events: &mut Vec<WebViewEvent>) {
        self.set_visible(false, events);
        self.view = None;
        self.bounds = None;
        self.visible = false;
        self.address = None;
        self.failed = false;
        while self.events.try_recv().is_ok() {}
    }

    fn set_bounds(
        &mut self,
        context: &egui::Context,
        rect: egui::Rect,
        events: &mut Vec<WebViewEvent>,
    ) {
        let scale = context.pixels_per_point();
        let bounds = Bounds {
            x: (rect.min.x * scale).round() as i32,
            y: (rect.min.y * scale).round() as i32,
            width: (rect.width() * scale).round().max(1.0) as u32,
            height: (rect.height() * scale).round().max(1.0) as u32,
        };
        if self.bounds == Some(bounds) {
            return;
        }
        let Some(view) = &self.view else {
            return;
        };
        if report(view.set_bounds(bounds), events) {
            self.bounds = Some(bounds);
        }
    }

    fn set_visible(&mut self, visible: bool, events: &mut Vec<WebViewEvent>) {
        if self.visible == visible {
            return;
        }
        let Some(view) = &self.view else {
            return;
        };
        if report(view.set_visible(visible), events) {
            self.visible = visible;
            if !visible {
                report(view.focus_parent(), events);
            }
        }
    }
}

fn report(result: Result<(), String>, events: &mut Vec<WebViewEvent>) -> bool {
    match result {
        Ok(()) => true,
        Err(error) => {
            events.push(WebViewEvent::Failed(error));
            false
        }
    }
}
