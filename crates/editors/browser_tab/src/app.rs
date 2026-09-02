use std::sync::Arc;

use block_client::blocks::web_browser_tab::{HistoryItem, WebBrowserTab, WebBrowserTabOperation};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::egui_material_icons::icons::{
    ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_REFRESH,
};
use block_editor_plugin::{egui, EditorHost, WebViewEvent};
use uuid::Uuid;

const INTRINSIC_SIZE: egui::Vec2 = egui::vec2(1024.0, 768.0);

#[derive(Default)]
pub struct BrowserTabApp {
    host: Option<EditorHost>,
    creation: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<WebBrowserTab>>,
    address: String,
    current: Option<String>,
    error: Option<String>,
    opened: bool,
    synchronized: Option<(usize, String)>,
    programmatic_navigation: Option<String>,
    natural_navigation: Option<String>,
}

impl BrowserTabApp {
    fn open(&mut self) {
        let (Some(host), Some(block)) = (self.host.as_ref(), self.block.as_ref()) else {
            return;
        };
        if self.opened {
            return;
        }
        let Some(url) = block.read().map(|tab| tab.current().url.clone()) else {
            return;
        };
        self.opened = true;
        self.address.clone_from(&url);
        self.current = Some(url.clone());
        self.programmatic_navigation = Some(url.clone());
        self.synchronized = block
            .read()
            .map(|tab| (tab.index(), tab.current().url.clone()));
        host.open_web_view(url);
    }

    fn process_events(&mut self) {
        let Some(host) = self.host.clone() else {
            return;
        };
        for event in host.take_web_view_events() {
            match event {
                WebViewEvent::Navigate(url) => self.navigation_started(url),
                WebViewEvent::Finished(url) => self.navigation_finished(url),
                WebViewEvent::Push(url) => self.push_item(url),
                WebViewEvent::Replace(url) => self.replace(url),
                WebViewEvent::Title(title) => self.title_changed(title),
                WebViewEvent::History(delta) => self.traverse_history(delta as isize),
                WebViewEvent::NewWindow(url) => self.push(url),
                WebViewEvent::Address(url) => self.current = Some(url),
                WebViewEvent::Failed(error) => self.error = Some(error),
            }
        }
    }

    fn push_item(&self, url: String) {
        if let Some(block) = self.block.as_ref() {
            block.operate(WebBrowserTabOperation::Push(self.item_with_url(url)));
        }
    }

    fn push(&self, url: String) {
        if let Some(block) = self.block.as_ref() {
            block.operate(WebBrowserTabOperation::Push(HistoryItem {
                url,
                title: String::new(),
            }));
        }
    }

    fn replace(&self, url: String) {
        if let Some(block) = self.block.as_ref() {
            block.operate(WebBrowserTabOperation::Replace(self.item_with_url(url)));
        }
    }

    fn navigation_started(&mut self, url: String) {
        if let Some(expected) = &self.programmatic_navigation {
            if expected == &url {
                return;
            }
            self.programmatic_navigation = Some(url.clone());
            self.replace(url);
            return;
        }
        if let Some(expected) = &self.natural_navigation {
            if expected == &url {
                return;
            }
            self.natural_navigation = Some(url.clone());
            self.replace(url);
            return;
        }
        self.natural_navigation = Some(url.clone());
        self.push(url);
    }

    fn navigation_finished(&mut self, url: String) {
        self.programmatic_navigation = None;
        self.natural_navigation = None;
        self.address.clone_from(&url);
        let current = self
            .block
            .as_ref()
            .and_then(|block| block.read())
            .map(|tab| tab.current().url.clone());
        if current.as_deref().is_some_and(|current| current != url) {
            self.replace(url);
        }
    }

    fn title_changed(&self, title: String) {
        let Some(block) = self.block.as_ref() else {
            return;
        };
        let Some(tab) = block.read() else {
            return;
        };
        let url = self
            .current
            .clone()
            .unwrap_or_else(|| tab.current().url.clone());
        drop(tab);
        block.operate(WebBrowserTabOperation::Replace(HistoryItem { url, title }));
    }

    fn item_with_url(&self, url: String) -> HistoryItem {
        let title = self
            .block
            .as_ref()
            .and_then(|block| block.read())
            .map(|tab| tab.current().title.clone())
            .unwrap_or_default();
        HistoryItem { url, title }
    }

    fn traverse_history(&self, delta: isize) {
        let Some(block) = self.block.as_ref() else {
            return;
        };
        let Some(tab) = block.read() else {
            return;
        };
        let Some(index) = tab.index().checked_add_signed(delta) else {
            return;
        };
        if index < tab.history().len() {
            drop(tab);
            block.operate(WebBrowserTabOperation::History(index));
        }
    }

    fn synchronize(&mut self) {
        let (Some(host), Some(block)) = (self.host.clone(), self.block.clone()) else {
            return;
        };
        let Some(tab) = block.read() else {
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
        if !natural && self.current.as_deref() != Some(url.as_str()) {
            self.programmatic_navigation = Some(url.clone());
            host.load_web_view(url);
        }
        self.synchronized = Some(selected);
    }
}

impl block_editor_plugin::App for BrowserTabApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.block = Some(client.get_block(block_id));
        self.address = "about:blank".into();
        self.host = Some(host);
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        Ok(client.create_block(WebBrowserTab::new()).id())
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        Some(INTRINSIC_SIZE)
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        self.process_events();
        let (Some(host), Some(block)) = (self.host.clone(), self.block.clone()) else {
            ui.spinner();
            return;
        };
        let Some(tab) = block.read() else {
            ui.spinner();
            return;
        };
        let back_index = tab.index().checked_sub(1);
        let forward_index = tab.can_go_forward().then_some(tab.index() + 1);
        drop(tab);

        let mut navigate = false;
        let mut back = false;
        let mut forward = false;
        let mut reload = false;
        let mut focus_app = false;
        ui.horizontal(|ui| {
            back = ui
                .add_enabled(back_index.is_some(), egui::Button::new(ICON_ARROW_BACK))
                .on_hover_text("Back")
                .test_id("browser.back")
                .clicked();
            forward = ui
                .add_enabled(
                    forward_index.is_some(),
                    egui::Button::new(ICON_ARROW_FORWARD),
                )
                .on_hover_text("Forward")
                .test_id("browser.forward")
                .clicked();
            reload = ui
                .button(ICON_REFRESH)
                .on_hover_text("Reload")
                .test_id("browser.reload")
                .clicked();

            let go_width = 32.0;
            let address_width =
                (ui.available_width() - go_width - ui.spacing().item_spacing.x).max(80.0);
            let address = ui
                .add_sized(
                    [address_width, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut self.address),
                )
                .test_id("browser.address");
            focus_app = address.clicked();
            navigate = ui
                .add_sized([go_width, 0.0], egui::Button::new("Go"))
                .test_id("browser.go")
                .clicked()
                || (address.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
        });

        if navigate {
            let url = browser_url(&self.address);
            self.address.clone_from(&url);
            self.push(url);
        } else if let Some(index) = back.then_some(back_index).flatten() {
            block.operate(WebBrowserTabOperation::History(index));
        } else if let Some(index) = forward.then_some(forward_index).flatten() {
            block.operate(WebBrowserTabOperation::History(index));
        } else if reload {
            host.reload_web_view();
        }
        if focus_app {
            host.focus_app();
        }
        self.synchronize();

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error)
                .test_id("browser.error");
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.process_events();
        self.open();
        self.synchronize();
        let Some(host) = self.host.clone() else {
            return;
        };
        let (response, _) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
        if self.error.is_some() {
            host.place_web_view(None);
            ui.painter().text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                self.error.clone().unwrap_or_default(),
                egui::FontId::proportional(14.0),
                ui.visuals().error_fg_color,
            );
            return;
        }
        let visible = response.rect.intersect(ui.clip_rect());
        host.place_web_view(visible.is_positive().then_some(visible));
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
