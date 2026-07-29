use std::sync::mpsc::{self, Receiver, Sender};

use block::{Block, BlockParent};
use block_client::{
    blocks::web_browser_tab::{HistoryItem, WebBrowserTab, WebBrowserTabOperation},
    BlockHandle, BlockRelationships,
};
use eframe::egui;
use uuid::Uuid;
use wry::{
    dpi::{PhysicalPosition, PhysicalSize},
    NewWindowResponse, PageLoadEvent, Rect as WebViewRect, WebView, WebViewBuilder,
};

use super::{BlockEditor, EditorAction};

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

enum BrowserEvent {
    Navigate(String),
    Finished(String),
    Push(String),
    Replace(String),
    Title(String),
    History(isize),
    NewWindow(String),
}

pub(super) struct WebBrowserTabEditor {
    block: BlockHandle<WebBrowserTab>,
    address: String,
    error: Option<String>,
    visible: bool,
    bounds: Option<WebViewRect>,
    webview: Option<WebView>,
    events: Receiver<BrowserEvent>,
    event_sender: Sender<BrowserEvent>,
    synchronized: Option<(usize, String)>,
    programmatic_navigation: Option<String>,
    natural_navigation: Option<String>,
    creation_failed: bool,
}

impl WebBrowserTabEditor {
    pub(super) fn new(block: BlockHandle<WebBrowserTab>) -> Self {
        let (event_sender, events) = mpsc::channel();
        Self {
            block,
            address: "about:blank".into(),
            error: None,
            visible: false,
            bounds: None,
            webview: None,
            events,
            event_sender,
            synchronized: None,
            programmatic_navigation: None,
            natural_navigation: None,
            creation_failed: false,
        }
    }

    fn ensure_webview(&mut self, frame: &eframe::Frame) {
        if self.webview.is_some() || self.creation_failed {
            return;
        }
        let Some(tab) = self.block.read() else {
            return;
        };
        let url = tab.current().url.clone();
        drop(tab);

        let navigation_events = self.event_sender.clone();
        let page_events = self.event_sender.clone();
        let ipc_events = self.event_sender.clone();
        let new_window_events = self.event_sender.clone();
        let title_events = self.event_sender.clone();
        let webview = WebViewBuilder::new()
            .with_url(&url)
            .with_visible(false)
            .with_focused(false)
            .with_bounds(WebViewRect {
                position: PhysicalPosition::new(0, 0).into(),
                size: PhysicalSize::new(1, 1).into(),
            })
            .with_initialization_script(HISTORY_SCRIPT)
            .with_navigation_handler(move |url| {
                let _ = navigation_events.send(BrowserEvent::Navigate(url));
                true
            })
            .with_on_page_load_handler(move |event, url| {
                if matches!(event, PageLoadEvent::Finished) {
                    let _ = page_events.send(BrowserEvent::Finished(url));
                }
            })
            .with_ipc_handler(move |request| {
                if let Some(event) = ipc_event(request.body()) {
                    let _ = ipc_events.send(event);
                }
            })
            .with_document_title_changed_handler(move |title| {
                let _ = title_events.send(BrowserEvent::Title(title));
            })
            .with_new_window_req_handler(move |url, _features| {
                let _ = new_window_events.send(BrowserEvent::NewWindow(url));
                NewWindowResponse::Deny
            })
            .build_as_child(frame);

        match webview {
            Ok(webview) => {
                self.address.clone_from(&url);
                self.programmatic_navigation = Some(url.clone());
                self.synchronized = self
                    .block
                    .read()
                    .map(|tab| (tab.index(), tab.current().url.clone()));
                self.webview = Some(webview);
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error.to_string());
                self.creation_failed = true;
            }
        }
    }

    fn process_events(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                BrowserEvent::Navigate(url) => self.navigation_started(url),
                BrowserEvent::Finished(url) => self.navigation_finished(url),
                BrowserEvent::Push(url) => {
                    self.block
                        .operate(WebBrowserTabOperation::Push(self.item_with_url(url)));
                }
                BrowserEvent::Replace(url) => {
                    self.block
                        .operate(WebBrowserTabOperation::Replace(self.item_with_url(url)));
                }
                BrowserEvent::Title(title) => self.title_changed(title),
                BrowserEvent::History(delta) => self.traverse_history(delta),
                BrowserEvent::NewWindow(url) => {
                    self.block
                        .operate(WebBrowserTabOperation::Push(HistoryItem {
                            url,
                            title: String::new(),
                        }));
                }
            }
        }
    }

    fn navigation_started(&mut self, url: String) {
        if let Some(expected) = &self.programmatic_navigation {
            if expected == &url {
                return;
            }
            self.programmatic_navigation = Some(url.clone());
            self.block
                .operate(WebBrowserTabOperation::Replace(self.item_with_url(url)));
            return;
        }
        if let Some(expected) = &self.natural_navigation {
            if expected == &url {
                return;
            }
            self.natural_navigation = Some(url.clone());
            self.block
                .operate(WebBrowserTabOperation::Replace(self.item_with_url(url)));
            return;
        }
        self.natural_navigation = Some(url.clone());
        self.block
            .operate(WebBrowserTabOperation::Push(HistoryItem {
                url,
                title: String::new(),
            }));
    }

    fn navigation_finished(&mut self, url: String) {
        self.programmatic_navigation = None;
        self.natural_navigation = None;
        self.address.clone_from(&url);
        let current = self.block.read().map(|tab| tab.current().url.clone());
        if current.as_deref().is_some_and(|current| current != url) {
            self.block
                .operate(WebBrowserTabOperation::Replace(self.item_with_url(url)));
        }
    }

    fn title_changed(&self, title: String) {
        let Some(tab) = self.block.read() else {
            return;
        };
        let url = self
            .webview
            .as_ref()
            .and_then(|webview| webview.url().ok())
            .unwrap_or_else(|| tab.current().url.clone());
        drop(tab);
        self.block
            .operate(WebBrowserTabOperation::Replace(HistoryItem { url, title }));
    }

    fn item_with_url(&self, url: String) -> HistoryItem {
        let title = self
            .block
            .read()
            .map(|tab| tab.current().title.clone())
            .unwrap_or_default();
        HistoryItem { url, title }
    }

    fn traverse_history(&self, delta: isize) {
        let Some(tab) = self.block.read() else {
            return;
        };
        let Some(index) = tab.index().checked_add_signed(delta) else {
            return;
        };
        if index < tab.history().len() {
            drop(tab);
            self.block.operate(WebBrowserTabOperation::History(index));
        }
    }

    fn synchronize(&mut self) {
        let Some(tab) = self.block.read() else {
            return;
        };
        let selected = (tab.index(), tab.current().url.clone());
        drop(tab);
        if self.synchronized.as_ref() == Some(&selected) {
            return;
        }

        let url = selected.1.clone();
        self.address.clone_from(&url);
        let natural = self.natural_navigation.as_deref() == Some(&url);
        let current = self.webview.as_ref().and_then(|webview| webview.url().ok());
        if !natural && current.as_deref() != Some(&url) {
            if let Some(webview) = &self.webview {
                self.programmatic_navigation = Some(url.clone());
                if let Err(error) = webview.load_url(&url) {
                    self.error = Some(error.to_string());
                    self.programmatic_navigation = None;
                }
            }
        }
        self.synchronized = Some(selected);
    }

    fn update_bounds(&mut self, context: &egui::Context, rect: egui::Rect) {
        let pixels_per_point = context.pixels_per_point();
        let bounds = WebViewRect {
            position: PhysicalPosition::new(
                (rect.min.x * pixels_per_point).round() as i32,
                (rect.min.y * pixels_per_point).round() as i32,
            )
            .into(),
            size: PhysicalSize::new(
                (rect.width() * pixels_per_point).round().max(1.0) as u32,
                (rect.height() * pixels_per_point).round().max(1.0) as u32,
            )
            .into(),
        };
        if self.bounds == Some(bounds) {
            return;
        }
        let Some(webview) = &self.webview else {
            return;
        };
        if let Err(error) = webview.set_bounds(bounds) {
            self.error = Some(error.to_string());
        } else {
            self.bounds = Some(bounds);
        }
    }

    fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        let Some(webview) = &self.webview else {
            return;
        };
        if let Err(error) = webview.set_visible(visible) {
            self.error = Some(error.to_string());
        } else {
            self.visible = visible;
            if !visible {
                self.focus_parent();
            }
        }
    }

    fn focus_parent(&mut self) {
        let Some(webview) = &self.webview else {
            return;
        };
        if let Err(error) = webview.focus_parent() {
            self.error = Some(error.to_string());
        }
    }

    fn close_webview(&mut self) {
        self.set_visible(false);
        self.webview = None;
        self.bounds = None;
        self.visible = false;
        self.synchronized = None;
        self.programmatic_navigation = None;
        self.natural_navigation = None;
        self.creation_failed = false;
        while self.events.try_recv().is_ok() {}
    }
}

impl BlockEditor for WebBrowserTabEditor {
    fn id(&self) -> Uuid {
        self.block.id()
    }

    fn block_type(&self) -> Uuid {
        WebBrowserTab::TYPE_ID
    }

    fn name(&self) -> String {
        self.block.name()
    }

    fn relationships(&self) -> Option<BlockRelationships> {
        self.block.read().map(|_| self.block.relationships())
    }

    fn set_parent(&self, parent: BlockParent) {
        self.block.set_parent(parent);
    }

    fn note_backref(&self, id: Uuid) {
        self.block.note_backref(id);
    }

    fn update_open_tab(&mut self, frame: &eframe::Frame) {
        self.ensure_webview(frame);
        self.process_events();
        self.synchronize();
    }

    fn set_tab_active(&mut self, active: bool) {
        if !active {
            self.set_visible(false);
        }
    }

    fn tab_closed(&mut self) {
        self.close_webview();
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _client: &block_client::BlockClient,
        frame: &eframe::Frame,
    ) -> Option<EditorAction> {
        self.ensure_webview(frame);
        self.process_events();
        self.synchronize();

        let Some(tab) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let back_index = tab.index().checked_sub(1);
        let forward_index = tab.can_go_forward().then_some(tab.index() + 1);
        drop(tab);

        let mut navigate = false;
        let mut back = false;
        let mut forward = false;
        let mut reload = false;
        let mut focus_parent = false;
        ui.horizontal(|ui| {
            back = ui
                .add_enabled(back_index.is_some(), egui::Button::new("\u{2190}"))
                .on_hover_text("Back")
                .clicked();
            forward = ui
                .add_enabled(forward_index.is_some(), egui::Button::new("\u{2192}"))
                .on_hover_text("Forward")
                .clicked();
            reload = ui.button("\u{21bb}").on_hover_text("Reload").clicked();

            let go_width = 32.0;
            let address_width =
                (ui.available_width() - go_width - ui.spacing().item_spacing.x).max(80.0);
            let address = ui.add_sized(
                [address_width, ui.spacing().interact_size.y],
                egui::TextEdit::singleline(&mut self.address),
            );
            focus_parent = address.clicked();
            navigate = ui
                .add_sized([go_width, 0.0], egui::Button::new("Go"))
                .clicked()
                || (address.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
        });

        if navigate {
            let url = browser_url(&self.address);
            self.address.clone_from(&url);
            self.block
                .operate(WebBrowserTabOperation::Push(HistoryItem {
                    url,
                    title: String::new(),
                }));
        } else if let Some(index) = back.then_some(back_index).flatten() {
            self.block.operate(WebBrowserTabOperation::History(index));
        } else if let Some(index) = forward.then_some(forward_index).flatten() {
            self.block.operate(WebBrowserTabOperation::History(index));
        } else if reload {
            if let Some(webview) = &self.webview {
                if let Err(error) = webview.reload() {
                    self.error = Some(error.to_string());
                }
            }
        }
        if focus_parent {
            self.focus_parent();
        }
        self.synchronize();

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.separator();
        let (response, _) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
        self.update_bounds(ui.ctx(), response.rect);
        self.set_visible(true);
        None
    }
}

fn ipc_event(message: &str) -> Option<BrowserEvent> {
    let (kind, value) = message.split_once(':')?;
    match kind {
        "push" => Some(BrowserEvent::Push(value.into())),
        "replace" => Some(BrowserEvent::Replace(value.into())),
        "history" => value.parse().ok().map(BrowserEvent::History),
        _ => None,
    }
}

fn browser_url(address: &str) -> String {
    let address = address.trim();
    if address.contains("://")
        || address.starts_with("about:")
        || address.starts_with("data:")
        || address.starts_with("file:")
    {
        address.to_owned()
    } else if address.starts_with("localhost") || address.starts_with("127.0.0.1") {
        format!("http://{address}")
    } else {
        format!("https://{address}")
    }
}
