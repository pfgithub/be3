use egui_material_icons::icons::{ICON_CHEVRON_RIGHT, ICON_CLOSE};

pub const COMPACT_FRAME_WIDTH: f32 = 760.0;

const SIDEBAR_DEFAULT_WIDTH: f32 = 240.0;
const SIDEBAR_MIN_WIDTH: f32 = 200.0;
const SIDEBAR_MAX_WIDTH: f32 = 340.0;
const FLOATING_SIDEBAR_MARGIN: f32 = 16.0;

pub trait FrameBands {
    fn toolbar_ui(&mut self, _ui: &mut egui::Ui) {}
    fn left_sidebar_ui(&mut self, _ui: &mut egui::Ui) {}
    fn right_sidebar_ui(&mut self, _ui: &mut egui::Ui) {}
    fn content_ui(&mut self, ui: &mut egui::Ui);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameRects {
    pub frame: egui::Rect,
    pub toolbar: Option<egui::Rect>,
    pub left_sidebar: Option<egui::Rect>,
    pub right_sidebar: Option<egui::Rect>,
    pub content_band: egui::Rect,
    pub content: egui::Rect,
    pub compact: bool,
}

impl Default for FrameRects {
    fn default() -> Self {
        Self {
            frame: egui::Rect::ZERO,
            toolbar: None,
            left_sidebar: None,
            right_sidebar: None,
            content_band: egui::Rect::ZERO,
            content: egui::Rect::ZERO,
            compact: false,
        }
    }
}

impl FrameRects {
    pub fn bands(&self) -> impl Iterator<Item = egui::Rect> + '_ {
        [self.toolbar, self.left_sidebar, self.right_sidebar]
            .into_iter()
            .flatten()
            .chain([self.content_band])
    }

    pub fn painted(&self) -> impl Iterator<Item = egui::Rect> + '_ {
        let sidebars = match self.compact {
            true => [None, None],
            false => [self.left_sidebar, self.right_sidebar],
        };
        [self.toolbar]
            .into_iter()
            .chain(sidebars)
            .flatten()
            .chain([self.content])
    }
}

#[derive(Debug, Default)]
pub struct FrameOutcome {
    pub rects: FrameRects,
    pub exit: bool,
}

pub struct Frame {
    id: egui::Id,
    toolbar: bool,
    left_sidebar: bool,
    right_sidebar: bool,
    read_only: bool,
    compact_width: f32,
    content: Option<egui::Rect>,
    trail: Vec<String>,
}

impl Frame {
    pub fn new(id: egui::Id) -> Self {
        Self {
            id,
            toolbar: false,
            left_sidebar: false,
            right_sidebar: false,
            read_only: false,
            compact_width: COMPACT_FRAME_WIDTH,
            content: None,
            trail: Vec::new(),
        }
    }

    pub fn toolbar(mut self, shown: bool) -> Self {
        self.toolbar = shown;
        self
    }

    pub fn left_sidebar(mut self, shown: bool) -> Self {
        self.left_sidebar = shown;
        self
    }

    pub fn right_sidebar(mut self, shown: bool) -> Self {
        self.right_sidebar = shown;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn content(mut self, rect: Option<egui::Rect>) -> Self {
        self.content = rect;
        self
    }

    pub fn trail(mut self, trail: Vec<String>) -> Self {
        self.trail = trail;
        self
    }

    pub fn show(self, ui: &mut egui::Ui, bands: &mut dyn FrameBands) -> FrameOutcome {
        let frame = ui.available_rect_before_wrap();
        let compact = frame.width() < self.compact_width;
        let mut outcome = FrameOutcome {
            rects: FrameRects {
                frame,
                compact,
                ..FrameRects::default()
            },
            exit: false,
        };
        if self.toolbar || !self.trail.is_empty() {
            let shown = egui::Panel::top(self.id.with("toolbar"))
                .show_separator_line(true)
                .show_inside(ui, |ui| self.toolbar_band(ui, bands));
            outcome.exit |= shown.inner;
            outcome.rects.toolbar = Some(shown.response.rect);
        }
        if compact {
            let available = ui.available_rect_before_wrap();
            if self.left_sidebar {
                egui::Window::new("Left sidebar")
                    .id(self.id.with("left-window"))
                    .default_width(SIDEBAR_DEFAULT_WIDTH)
                    .resizable(true)
                    .default_pos(available.left_top() + egui::Vec2::splat(FLOATING_SIDEBAR_MARGIN))
                    .show(ui.ctx(), |ui| {
                        outcome.rects.left_sidebar = Some(ui.max_rect());
                        scrolled(ui, self.read_only, |ui| bands.left_sidebar_ui(ui));
                    });
            }
            if self.right_sidebar {
                egui::Window::new("Right sidebar")
                    .id(self.id.with("right-window"))
                    .pivot(egui::Align2::RIGHT_TOP)
                    .default_width(SIDEBAR_DEFAULT_WIDTH)
                    .resizable(true)
                    .default_pos(
                        available.right_top()
                            + egui::vec2(-FLOATING_SIDEBAR_MARGIN, FLOATING_SIDEBAR_MARGIN),
                    )
                    .show(ui.ctx(), |ui| {
                        outcome.rects.right_sidebar = Some(ui.max_rect());
                        scrolled(ui, self.read_only, |ui| bands.right_sidebar_ui(ui));
                    });
            }
        } else {
            if self.left_sidebar {
                let shown = sidebar(egui::Panel::left(self.id.with("left")))
                    .show_inside(ui, |ui| {
                        read_only_scope(ui, self.read_only, |ui| bands.left_sidebar_ui(ui))
                    });
                outcome.rects.left_sidebar = Some(shown.response.rect);
            }
            if self.right_sidebar {
                let shown = sidebar(egui::Panel::right(self.id.with("right")))
                    .show_inside(ui, |ui| {
                        read_only_scope(ui, self.read_only, |ui| bands.right_sidebar_ui(ui))
                    });
                outcome.rects.right_sidebar = Some(shown.response.rect);
            }
        }
        let band = ui.available_rect_before_wrap();
        outcome.rects.content_band = band;
        let content = self.content.map_or(band, |rect| rect.intersect(band));
        outcome.rects.content = content;
        let mut inner = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(self.id.with("content"))
                .max_rect(content)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        inner.set_clip_rect(content.intersect(ui.clip_rect()));
        bands.content_ui(&mut inner);
        ui.advance_cursor_after_rect(band);
        outcome
    }

    fn toolbar_band(&self, ui: &mut egui::Ui, bands: &mut dyn FrameBands) -> bool {
        if self.trail.is_empty() {
            read_only_scope(ui, self.read_only, |ui| bands.toolbar_ui(ui));
            return false;
        }
        let band = ui.available_rect_before_wrap();
        let mut crumbs = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(self.id.with("breadcrumb"))
                .max_rect(band)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let exit = breadcrumb(&mut crumbs, &self.trail);
        let used = crumbs.min_rect();
        let rest = band.with_min_x((used.right() + ui.spacing().item_spacing.x).min(band.right()));
        let mut inner = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(self.id.with("toolbar-contents"))
                .max_rect(rest)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        inner.set_clip_rect(rest.intersect(ui.clip_rect()));
        read_only_scope(&mut inner, self.read_only, |ui| bands.toolbar_ui(ui));
        let height = used.height().max(inner.min_rect().height());
        ui.advance_cursor_after_rect(egui::Rect::from_min_size(
            band.min,
            egui::vec2(band.width(), height),
        ));
        exit || ui
            .ctx()
            .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    }
}

pub fn read_only_scope<R>(
    ui: &mut egui::Ui,
    read_only: bool,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    if !read_only {
        return contents(ui);
    }
    let mut style = (**ui.style()).clone();
    style.visuals.disabled_alpha = 1.0;
    ui.scope_builder(egui::UiBuilder::new().style(style).disabled(), contents)
        .inner
}

fn sidebar(panel: egui::Panel) -> egui::Panel {
    panel
        .default_size(SIDEBAR_DEFAULT_WIDTH)
        .min_size(SIDEBAR_MIN_WIDTH)
        .max_size(SIDEBAR_MAX_WIDTH)
        .resizable(true)
}

fn scrolled<R>(ui: &mut egui::Ui, read_only: bool, contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| read_only_scope(ui, read_only, contents))
        .inner
}

fn breadcrumb(ui: &mut egui::Ui, trail: &[String]) -> bool {
    let mut exit = false;
    ui.horizontal(|ui| {
        for (index, step) in trail.iter().enumerate() {
            if index > 0 {
                ui.add(egui::Label::new(ICON_CHEVRON_RIGHT.codepoint).selectable(false));
            }
            let last = index + 1 == trail.len();
            let text = egui::RichText::new(step);
            ui.add(
                egui::Label::new(match last {
                    true => text.strong(),
                    false => text.weak(),
                })
                .selectable(false),
            );
        }
        exit = ui
            .button(ICON_CLOSE.codepoint)
            .on_hover_text("Leave this editor (Escape)")
            .clicked();
        ui.separator();
    });
    exit
}

#[cfg(test)]
mod tests;
