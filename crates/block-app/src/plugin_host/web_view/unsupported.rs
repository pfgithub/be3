use std::sync::mpsc::Sender;

use block_plugin_api::WebViewEvent;

use super::Bounds;

pub(super) enum WebView {}

impl WebView {
    pub(super) fn new(
        _frame: &eframe::Frame,
        _url: &str,
        _events: &Sender<WebViewEvent>,
    ) -> Result<Self, String> {
        Err("The embedded browser is not supported on this platform.".to_owned())
    }

    pub(super) fn url(&self) -> Option<String> {
        match *self {}
    }

    pub(super) fn load_url(&self, _url: &str) -> Result<(), String> {
        match *self {}
    }

    pub(super) fn reload(&self) -> Result<(), String> {
        match *self {}
    }

    pub(super) fn set_bounds(&self, _bounds: Bounds) -> Result<(), String> {
        match *self {}
    }

    pub(super) fn set_visible(&self, _visible: bool) -> Result<(), String> {
        match *self {}
    }

    pub(super) fn focus_parent(&self) -> Result<(), String> {
        match *self {}
    }
}
