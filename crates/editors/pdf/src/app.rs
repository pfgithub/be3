use std::{collections::HashMap, sync::Arc, time::Duration};

use block_client::{
    blocks::pdf::{Pdf, PdfOperation},
    BlockClient, BlockHandle,
};
use block_editor_plugin::{
    egui,
    egui_material_icons::icons::{ICON_ARROW_BACK, ICON_ARROW_FORWARD},
    EditorHost, FileFilter, FilePicker, PickedFile,
};
use uuid::Uuid;

use crate::pane::Pane;

const DEFAULT_PAGE_SIZE: egui::Vec2 = egui::vec2(612.0, 792.0);
const LOADING_FILL: egui::Color32 = egui::Color32::from_gray(35);
const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 32.0;
const RENDER_POLL: Duration = Duration::from_millis(30);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum PaneKey {
    Main,
    Preview,
}

#[derive(Clone, Copy)]
struct View {
    zoom: f32,
    pan: egui::Vec2,
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
        }
    }
}

enum Drawn {
    Page,
    Waiting,
    Failed(String),
}

struct Editing {
    host: EditorHost,
    _client: Arc<BlockClient>,
    block: BlockHandle<Pdf>,
}

struct Creating {
    host: EditorHost,
    client: BlockClient,
    chosen: Option<Pdf>,
}

#[derive(Default)]
pub struct PdfApp {
    editing: Option<Editing>,
    creation: Option<Creating>,
    picker: FilePicker,
    error: Option<String>,
    page: usize,
    page_count: Option<usize>,
    page_size_pts: Option<egui::Vec2>,
    view: View,
    panes: HashMap<PaneKey, Pane>,
}

impl block_editor_plugin::App for PdfApp {
    fn connect(&mut self, host: EditorHost, client: BlockClient, block_id: Uuid) {
        let client = Arc::new(client);
        let block = client.get_block::<Pdf>(block_id);
        self.editing = Some(Editing {
            host,
            _client: client,
            block,
        });
    }

    fn connect_creation(&mut self, host: EditorHost, client: BlockClient) {
        self.creation = Some(Creating {
            host,
            client,
            chosen: None,
        });
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let creation = self
            .creation
            .as_mut()
            .ok_or("this editor is not filling in a block")?;
        let pdf = creation.chosen.take().ok_or("no file was chosen")?;
        Ok(creation.client.create_block(pdf).id())
    }

    fn creation_ui(&mut self, ui: &mut egui::Ui) {
        let Some(Creating { host, chosen, .. }) = &mut self.creation else {
            return;
        };
        match self.picker.poll(host).map(|file| file.and_then(imported)) {
            Some(Ok(pdf)) => {
                host.set_creation_ready(true);
                *chosen = Some(pdf);
                self.error = None;
            }
            Some(Err(error)) => {
                host.set_creation_ready(false);
                *chosen = None;
                self.error = Some(error);
            }
            None => {}
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.picker.is_open(), egui::Button::new("Choose file..."))
                .clicked()
            {
                self.picker.open(host, filter());
            }
            match chosen {
                Some(pdf) => ui.label(pdf.source_name()),
                None => ui.weak("No file chosen"),
            };
        });
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size().max(egui::Vec2::splat(1.0));
        let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
        self.handle_input(&response, rect);
        let view = self.view;
        match self.draw(ui, PaneKey::Main, rect, view) {
            Drawn::Page => {}
            waiting => {
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    ui.centered_and_justified(|ui| match waiting {
                        Drawn::Failed(error) => {
                            ui.colored_label(ui.visuals().error_fg_color, error);
                        }
                        _ => {
                            ui.spinner();
                        }
                    });
                });
            }
        }
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, egui::Sense::hover());
        if !matches!(
            self.draw(ui, PaneKey::Preview, rect, View::default()),
            Drawn::Page
        ) {
            ui.painter().rect_filled(rect, 0.0, LOADING_FILL);
        }
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.strong("PDF");
            if ui
                .add_enabled(self.page > 0, egui::Button::new(ICON_ARROW_BACK))
                .on_hover_text("Previous page")
                .clicked()
            {
                self.page -= 1;
            }
            let label = match self.page_count {
                Some(count) => format!("Page {} of {count}", self.page + 1),
                None => "Loading...".to_owned(),
            };
            ui.label(label);
            let has_next = self.page_count.is_none_or(|count| self.page + 1 < count);
            if ui
                .add_enabled(has_next, egui::Button::new(ICON_ARROW_FORWARD))
                .on_hover_text("Next page")
                .clicked()
            {
                self.page += 1;
            }
            ui.separator();
            if ui.button("Fit view").clicked() {
                self.view = View::default();
            }
        });
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("PDF");
        let Some(editing) = &self.editing else {
            return;
        };
        if let Some(pdf) = editing.block.read() {
            ui.label(pdf.source_name());
        }
        let editable = editing.host.editable();
        match self
            .picker
            .poll(&editing.host)
            .map(|file| file.and_then(imported))
        {
            Some(Ok(pdf)) => {
                editing.block.operate(PdfOperation::Replace { pdf });
                self.page = 0;
                self.error = None;
            }
            Some(Err(error)) => self.error = Some(error),
            None => {}
        }
        if ui
            .add_enabled(
                editable && !self.picker.is_open(),
                egui::Button::new("Replace PDF..."),
            )
            .clicked()
        {
            self.picker.open(&editing.host, filter());
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        Some(self.page_size())
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        let size = self.page_size_pts?;
        (size.y != 0.0).then(|| size.x / size.y)
    }
}

impl PdfApp {
    fn page_size(&self) -> egui::Vec2 {
        self.page_size_pts.unwrap_or(DEFAULT_PAGE_SIZE)
    }

    fn draw(&mut self, ui: &mut egui::Ui, key: PaneKey, region: egui::Rect, view: View) -> Drawn {
        let Some(revision) = self
            .editing
            .as_ref()
            .map(|editing| editing.block.revision())
        else {
            return Drawn::Waiting;
        };
        let mut pane = self.panes.remove(&key).unwrap_or_default();
        if let Some(facts) = pane.poll(ui.ctx()) {
            self.page_count = Some(facts.page_count);
            if self.page >= facts.page_count {
                self.page = facts.page_index;
            }
            self.page_size_pts = Some(facts.page_size_pts);
        }
        let page_size = self.page_size();
        let page_rect = page_rect(region, page_size, view);
        let editing = self.editing.as_ref();
        pane.ensure(
            revision,
            self.page,
            self.page_size_pts,
            page_rect,
            page_rect.intersect(ui.clip_rect()),
            ui.ctx().pixels_per_point(),
            || {
                let pdf = editing?.block.read()?;
                Some(pdf.data().to_vec())
            },
        );
        if pane.is_rendering() {
            ui.ctx().request_repaint_after(RENDER_POLL);
        }
        let drawn = if pane.has_page() {
            pane.paint(&ui.painter_at(region), page_rect, page_size);
            Drawn::Page
        } else {
            match pane.error() {
                Some(error) => Drawn::Failed(error.to_owned()),
                None => Drawn::Waiting,
            }
        };
        self.panes.insert(key, pane);
        drawn
    }

    fn handle_input(&mut self, response: &egui::Response, region: egui::Rect) {
        if response.dragged() {
            self.view.pan += response.drag_delta();
        }
        if !response.hovered() {
            return;
        }
        let Some(pointer) = response.ctx.pointer_hover_pos() else {
            return;
        };
        let (scroll, zoom_delta, command) = response.ctx.input(|input| {
            (
                input.smooth_scroll_delta,
                input.zoom_delta(),
                input.modifiers.command,
            )
        });
        if (zoom_delta - 1.0).abs() > f32::EPSILON {
            self.zoom(zoom_delta, pointer, region);
        } else if command && scroll.y != 0.0 {
            self.zoom((scroll.y * 0.002).exp(), pointer, region);
        } else if scroll != egui::Vec2::ZERO {
            self.view.pan += scroll;
        }
    }

    fn zoom(&mut self, factor: f32, anchor: egui::Pos2, region: egui::Rect) {
        let old = self.view.zoom;
        let new = (old * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        if new == old {
            return;
        }
        let offset = anchor - region.center();
        self.view.pan = offset - (offset - self.view.pan) * (new / old);
        self.view.zoom = new;
    }
}

fn page_rect(region: egui::Rect, page_size: egui::Vec2, view: View) -> egui::Rect {
    let scale = (region.width() / page_size.x)
        .min(region.height() / page_size.y)
        .max(f32::EPSILON);
    egui::Rect::from_center_size(region.center() + view.pan, page_size * scale * view.zoom)
}

fn filter() -> FileFilter {
    FileFilter {
        name: "PDF".to_owned(),
        default_file_name: "Document.pdf".to_owned(),
        extensions: vec!["pdf".to_owned()],
        mime_types: vec!["application/pdf".to_owned()],
    }
}

fn imported(file: PickedFile) -> Result<Pdf, String> {
    let PickedFile { name, data } = file;
    Pdf::new(name.clone(), data).map_err(|error| format!("Could not import {name}: {error}"))
}
