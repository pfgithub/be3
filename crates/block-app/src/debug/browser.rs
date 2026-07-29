use std::io;

use eframe::egui;
use wry::{
    dpi::{PhysicalPosition, PhysicalSize},
    Rect as WebViewRect, WebView, WebViewBuilder,
};

pub(crate) struct BrowserDebug {
    open: bool,
    url: String,
    error: Option<String>,
    visible: bool,
    bounds: Option<WebViewRect>,
    webview: WebView,
}

impl BrowserDebug {
    pub(crate) fn new(creation_context: &eframe::CreationContext<'_>) -> io::Result<Self> {
        let url = "https://example.com".to_owned();
        let webview = WebViewBuilder::new()
            .with_url(&url)
            .with_visible(false)
            .with_focused(false)
            .with_bounds(WebViewRect {
                position: PhysicalPosition::new(0, 0).into(),
                size: PhysicalSize::new(1, 1).into(),
            })
            .build_as_child(creation_context)
            .map_err(|error| io::Error::other(error.to_string()))?;

        Ok(Self {
            open: false,
            url,
            error: None,
            visible: false,
            bounds: None,
            webview,
        })
    }

    pub(crate) fn open(&mut self) {
        self.open = true;
    }

    pub(crate) fn show(&mut self, ctx: &egui::Context) {
        let mut open = self.open;
        let mut navigate = false;
        let mut back = false;
        let mut forward = false;
        let mut reload = false;
        let mut focus_parent = false;

        let shown = egui::Window::new("Web Browser")
            .open(&mut open)
            .default_size([800.0, 560.0])
            .min_size([320.0, 240.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    back = ui.button("\u{2190}").on_hover_text("Back").clicked();
                    forward = ui.button("\u{2192}").on_hover_text("Forward").clicked();
                    reload = ui.button("\u{21bb}").on_hover_text("Reload").clicked();

                    let go_width = 32.0;
                    let address_width =
                        (ui.available_width() - go_width - ui.spacing().item_spacing.x).max(80.0);
                    let address = ui.add_sized(
                        [address_width, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(&mut self.url),
                    );
                    focus_parent = address.clicked();
                    navigate = ui
                        .add_sized([go_width, 0.0], egui::Button::new("Go"))
                        .clicked()
                        || (address.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                });
                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.separator();
                let (response, _) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
                response.rect
            });

        self.open = open;
        if focus_parent {
            self.focus_parent();
        }

        let operation = if navigate {
            let url = browser_url(&self.url);
            self.url.clone_from(&url);
            self.webview.load_url(&url)
        } else if back {
            self.webview.evaluate_script("history.back()")
        } else if forward {
            self.webview.evaluate_script("history.forward()")
        } else if reload {
            self.webview.reload()
        } else {
            Ok(())
        };
        if let Err(error) = operation {
            self.error = Some(error.to_string());
        } else if navigate || back || forward || reload {
            self.error = None;
        }

        let browser_rect = shown.and_then(|response| response.inner);
        if open {
            if let Some(rect) = browser_rect {
                self.update_bounds(ctx, rect);
                self.set_visible(true);
                return;
            }
        }
        self.set_visible(false);
    }

    fn update_bounds(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        let pixels_per_point = ctx.pixels_per_point();
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
        if let Err(error) = self.webview.set_bounds(bounds) {
            self.error = Some(error.to_string());
        } else {
            self.bounds = Some(bounds);
        }
    }

    fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        if let Err(error) = self.webview.set_visible(visible) {
            self.error = Some(error.to_string());
        } else {
            self.visible = visible;
            if !visible {
                self.focus_parent();
            }
        }
    }

    fn focus_parent(&mut self) {
        if let Err(error) = self.webview.focus_parent() {
            self.error = Some(error.to_string());
        }
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
