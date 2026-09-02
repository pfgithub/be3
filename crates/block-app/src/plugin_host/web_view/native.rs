use std::sync::mpsc::Sender;

use block_plugin_api::WebViewEvent;
use wry::{
    dpi::{PhysicalPosition, PhysicalSize},
    NewWindowResponse, PageLoadEvent, Rect as WebViewRect, WebViewBuilder,
};

use super::Bounds;

const HISTORY_SCRIPT: &str = r#"
(() => {
    const send = (kind, value) => window.ipc.postMessage(`${kind}:${value}`);
    const pushState = history.pushState.bind(history);
    const replaceState = history.replaceState.bind(history);

    history.pushState = (...args) => {
        const result = pushState(...args);
        send("push", location.href);
        return result;
    };
    history.replaceState = (...args) => {
        const result = replaceState(...args);
        send("replace", location.href);
        return result;
    };
    history.go = (delta = 0) => send("history", Number(delta) || 0);
    history.back = () => send("history", -1);
    history.forward = () => send("history", 1);
})();
"#;

pub(super) struct WebView {
    webview: wry::WebView,
}

impl WebView {
    pub(super) fn new(
        frame: &eframe::Frame,
        url: &str,
        events: &Sender<WebViewEvent>,
    ) -> Result<Self, String> {
        let navigation_events = events.clone();
        let page_events = events.clone();
        let ipc_events = events.clone();
        let new_window_events = events.clone();
        let title_events = events.clone();
        WebViewBuilder::new()
            .with_url(url)
            .with_visible(false)
            .with_focused(false)
            .with_bounds(WebViewRect {
                position: PhysicalPosition::new(0, 0).into(),
                size: PhysicalSize::new(1, 1).into(),
            })
            .with_initialization_script(HISTORY_SCRIPT)
            .with_navigation_handler(move |url| {
                let _ = navigation_events.send(WebViewEvent::Navigate(url));
                true
            })
            .with_on_page_load_handler(move |event, url| {
                if matches!(event, PageLoadEvent::Finished) {
                    let _ = page_events.send(WebViewEvent::Finished(url));
                }
            })
            .with_ipc_handler(move |request| {
                if let Some(event) = ipc_event(request.body()) {
                    let _ = ipc_events.send(event);
                }
            })
            .with_document_title_changed_handler(move |title| {
                let _ = title_events.send(WebViewEvent::Title(title));
            })
            .with_new_window_req_handler(move |url, _features| {
                let _ = new_window_events.send(WebViewEvent::NewWindow(url));
                NewWindowResponse::Deny
            })
            .build_as_child(frame)
            .map(|webview| Self { webview })
            .map_err(|error| error.to_string())
    }

    pub(super) fn url(&self) -> Option<String> {
        self.webview.url().ok()
    }

    pub(super) fn load_url(&self, url: &str) -> Result<(), String> {
        self.webview
            .load_url(url)
            .map_err(|error| error.to_string())
    }

    pub(super) fn reload(&self) -> Result<(), String> {
        self.webview.reload().map_err(|error| error.to_string())
    }

    pub(super) fn set_bounds(&self, bounds: Bounds) -> Result<(), String> {
        self.webview
            .set_bounds(WebViewRect {
                position: PhysicalPosition::new(bounds.x, bounds.y).into(),
                size: PhysicalSize::new(bounds.width, bounds.height).into(),
            })
            .map_err(|error| error.to_string())
    }

    pub(super) fn set_visible(&self, visible: bool) -> Result<(), String> {
        self.webview
            .set_visible(visible)
            .map_err(|error| error.to_string())
    }

    pub(super) fn focus_parent(&self) -> Result<(), String> {
        self.webview
            .focus_parent()
            .map_err(|error| error.to_string())
    }
}

fn ipc_event(message: &str) -> Option<WebViewEvent> {
    let (kind, value) = message.split_once(':')?;
    match kind {
        "push" => Some(WebViewEvent::Push(value.into())),
        "replace" => Some(WebViewEvent::Replace(value.into())),
        "history" => value.parse().ok().map(WebViewEvent::History),
        _ => None,
    }
}
