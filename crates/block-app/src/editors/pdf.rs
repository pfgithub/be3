use std::{
    sync::{
        mpsc::{self, Receiver, TryRecvError},
        OnceLock,
    },
    thread,
};

use block_client::{
    blocks::pdf::{Pdf, PdfOperation},
    BlockClient, BlockHandle,
};
use eframe::egui::{self, Color32, Pos2, Sense, TextureHandle, Vec2};
use egui_material_icons::{
    icons::{ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_PICTURE_AS_PDF},
    MaterialIcon,
};
use pdfium_render::prelude::{PdfRenderConfig, Pdfium};

use super::{
    BlockEditor, ConfigurableEditor, CreationOptions, DirectEditorCapabilities,
    DirectEditorViewport, EditorAccess, EditorAction, EditorKind,
};

const DEFAULT_PAGE_SIZE: Vec2 = egui::vec2(850.0, 1100.0);
const RENDER_MAX_DIM: u32 = 4096;
const TILE_MARGIN_FACTOR: f32 = 0.5;
const TILE_MIN_SCALE_FACTOR: f32 = 0.4;
const TILE_MAX_SCALE_FACTOR: f32 = 1.15;

impl EditorKind for PdfEditor {
    type Block = Pdf;

    const DISPLAY_NAME: &'static str = "PDF";
    const ICON: MaterialIcon = ICON_PICTURE_AS_PDF;

    fn open(_client: &BlockClient, block: BlockHandle<Pdf>) -> Self {
        Self::new(block)
    }
}

impl ConfigurableEditor for PdfEditor {
    type Options = ChosenPdf;

    fn create(client: &BlockClient, options: ChosenPdf) -> Result<Self, String> {
        let pdf = options.pdf.ok_or("Choose a PDF file first")?;
        Ok(Self::new(client.create_block(pdf)))
    }
}

#[derive(Default)]
pub(crate) struct ChosenPdf {
    pdf: Option<Pdf>,
    error: Option<String>,
}

impl CreationOptions for ChosenPdf {
    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        ui.horizontal(|ui| {
            if ui.button("Choose file...").clicked() {
                match pick_pdf_file() {
                    Ok(Some(pdf)) => {
                        self.pdf = Some(pdf);
                        self.error = None;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        self.pdf = None;
                        self.error = Some(error);
                    }
                }
            }
            match &self.pdf {
                Some(pdf) => ui.label(pdf.source_name()),
                None => ui.weak("No file chosen"),
            };
        });
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        self.pdf.is_some()
    }
}

struct RenderedPage {
    page_count: usize,
    page_index: usize,
    page_size_pts: Vec2,
    tile_origin_pts: Pos2,
    tile_size_pts: Vec2,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

#[derive(Clone, Copy)]
struct RenderRequest {
    revision: u64,
    page: usize,
    scale: f32,
    origin_pts: Pos2,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy)]
struct TileCoverage {
    revision: u64,
    page: usize,
    scale: f32,
    origin_pts: Pos2,
    size_pts: Vec2,
}

impl TileCoverage {
    fn satisfies(&self, revision: u64, page: usize, scale: f32, origin: Pos2, size: Vec2) -> bool {
        if self.revision != revision || self.page != page {
            return false;
        }
        if scale > self.scale * TILE_MAX_SCALE_FACTOR || scale < self.scale * TILE_MIN_SCALE_FACTOR
        {
            return false;
        }
        const EPS: f32 = 0.5;
        let visible = egui::Rect::from_min_size(origin, size);
        let tile = egui::Rect::from_min_size(self.origin_pts, self.size_pts);
        visible.min.x >= tile.min.x - EPS
            && visible.min.y >= tile.min.y - EPS
            && visible.max.x <= tile.max.x + EPS
            && visible.max.y <= tile.max.y + EPS
    }
}

type RenderJobResult = Result<RenderedPage, String>;

pub(crate) struct PdfEditor {
    block: BlockHandle<Pdf>,
    page: usize,
    page_count: Option<usize>,
    page_size_pts: Option<Vec2>,
    texture: Option<TextureHandle>,
    tile: Option<TileCoverage>,
    render_job: Option<(RenderRequest, Receiver<RenderJobResult>)>,
    failed_request: Option<TileCoverage>,
    render_error: Option<String>,
    import_error: Option<String>,
}

impl PdfEditor {
    pub(crate) fn new(block: BlockHandle<Pdf>) -> Self {
        Self {
            block,
            page: 0,
            page_count: None,
            page_size_pts: None,
            texture: None,
            tile: None,
            render_job: None,
            failed_request: None,
            render_error: None,
            import_error: None,
        }
    }

    fn ensure_texture(
        &mut self,
        context: &egui::Context,
        page_rect: egui::Rect,
        visible_rect: egui::Rect,
        pixels_per_point: f32,
    ) -> bool {
        if let Some((request, receiver)) = &self.render_job {
            match receiver.try_recv() {
                Ok(result) => {
                    let request = *request;
                    self.render_job = None;
                    match result {
                        Ok(rendered) => {
                            self.page_count = Some(rendered.page_count);
                            if self.page >= rendered.page_count {
                                self.page = rendered.page_index;
                            }
                            self.page_size_pts = Some(rendered.page_size_pts);
                            self.render_error = None;
                            self.failed_request = None;
                            let image = egui::ColorImage::from_rgba_unmultiplied(
                                [rendered.width as usize, rendered.height as usize],
                                &rendered.rgba,
                            );
                            if let Some(texture) = &mut self.texture {
                                texture.set(image, egui::TextureOptions::LINEAR);
                            } else {
                                self.texture = Some(context.load_texture(
                                    format!("pdf-block-{}", self.block.id()),
                                    image,
                                    egui::TextureOptions::LINEAR,
                                ));
                            }
                            self.tile = Some(TileCoverage {
                                revision: request.revision,
                                page: rendered.page_index,
                                scale: request.scale,
                                origin_pts: rendered.tile_origin_pts,
                                size_pts: rendered.tile_size_pts,
                            });
                        }
                        Err(error) => {
                            self.render_error = Some(error);
                            self.failed_request = Some(TileCoverage {
                                revision: request.revision,
                                page: request.page,
                                scale: request.scale,
                                origin_pts: request.origin_pts,
                                size_pts: egui::vec2(
                                    request.width as f32 / request.scale,
                                    request.height as f32 / request.scale,
                                ),
                            });
                        }
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.render_job = None,
            }
        }

        let Some(pdf) = self.block.read() else {
            return false;
        };
        let revision = self.block.revision();
        drop(pdf);

        if visible_rect.width() <= 0.0 || visible_rect.height() <= 0.0 || page_rect.width() <= 0.0 {
            return self.texture.is_some();
        }

        let page_size_pts = self.page_size_pts.unwrap_or(DEFAULT_PAGE_SIZE);
        let screen_pts_per_pdf_pt = page_rect.width() / page_size_pts.x;
        let scale = (screen_pts_per_pdf_pt * pixels_per_point).max(0.001);
        let inv = 1.0 / screen_pts_per_pdf_pt;
        let origin_pts = Pos2::new(
            (visible_rect.min.x - page_rect.min.x) * inv,
            (visible_rect.min.y - page_rect.min.y) * inv,
        );
        let size_pts = egui::vec2(visible_rect.width() * inv, visible_rect.height() * inv);

        let satisfied = self
            .tile
            .is_some_and(|tile| tile.satisfies(revision, self.page, scale, origin_pts, size_pts));
        if satisfied {
            return self.texture.is_some();
        }

        let already_failed = self.failed_request.is_some_and(|failed| {
            failed.satisfies(revision, self.page, scale, origin_pts, size_pts)
        });
        if already_failed {
            return self.texture.is_some();
        }

        if self.render_job.is_none() {
            let pdf = self.block.read().expect("checked above");
            let data = pdf.data().to_vec();
            drop(pdf);

            let margin = size_pts * TILE_MARGIN_FACTOR;
            let mut padded_origin = origin_pts - margin;
            let mut padded_size = size_pts + margin * 2.0;
            padded_origin.x = padded_origin.x.max(0.0).min(page_size_pts.x);
            padded_origin.y = padded_origin.y.max(0.0).min(page_size_pts.y);
            padded_size.x = padded_size
                .x
                .min(page_size_pts.x - padded_origin.x)
                .max(1.0);
            padded_size.y = padded_size
                .y
                .min(page_size_pts.y - padded_origin.y)
                .max(1.0);

            let width = ((padded_size.x * scale).round() as u32).clamp(1, RENDER_MAX_DIM);
            let height = ((padded_size.y * scale).round() as u32).clamp(1, RENDER_MAX_DIM);

            let request = RenderRequest {
                revision,
                page: self.page,
                scale,
                origin_pts: padded_origin,
                width,
                height,
            };
            let repaint = context.clone();
            let page = self.page;
            let (sender, receiver) = mpsc::channel();
            thread::Builder::new()
                .name("pdf-render".into())
                .spawn(move || {
                    let result = render_page(&data, page, scale, padded_origin, width, height);
                    let _ = sender.send(result);
                    repaint.request_repaint();
                })
                .expect("failed to start pdf render job");
            self.render_job = Some((request, receiver));
        }
        self.texture.is_some()
    }

    fn paint_texture(&self, ui: &mut egui::Ui, page_rect: egui::Rect) -> egui::Response {
        let available = ui.available_size().max(Vec2::splat(1.0));
        let (rect, response) = ui.allocate_exact_size(available, Sense::drag());
        match (&self.texture, self.tile) {
            (Some(texture), Some(tile)) => {
                let page_size_pts = self.page_size_pts.unwrap_or(DEFAULT_PAGE_SIZE);
                let screen_pts_per_pdf_pt = page_rect.width() / page_size_pts.x;
                let tile_rect = egui::Rect::from_min_size(
                    page_rect.min + tile.origin_pts.to_vec2() * screen_pts_per_pdf_pt,
                    tile.size_pts * screen_pts_per_pdf_pt,
                );
                let painter = ui.painter_at(rect);
                painter.rect_filled(page_rect.intersect(rect), 0.0, Color32::WHITE);
                painter.image(
                    texture.id(),
                    tile_rect.intersect(rect),
                    egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            _ => {
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    ui.centered_and_justified(|ui| {
                        if let Some(error) = &self.render_error {
                            ui.colored_label(ui.visuals().error_fg_color, error);
                        } else {
                            ui.spinner();
                        }
                    });
                });
            }
        }
        response
    }

    fn replace_from_file(&mut self) {
        match pick_pdf_file() {
            Ok(Some(pdf)) => {
                self.block.operate(PdfOperation::Replace { pdf });
                self.page = 0;
                self.import_error = None;
            }
            Ok(None) => {}
            Err(error) => self.import_error = Some(error),
        }
    }
}

fn render_page(
    data: &[u8],
    page: usize,
    scale: f32,
    origin_pts: Pos2,
    width: u32,
    height: u32,
) -> Result<RenderedPage, String> {
    let pdfium = pdfium_instance().map_err(str::to_owned)?;
    let document = pdfium
        .load_pdf_from_byte_slice(data, None)
        .map_err(|error| error.to_string())?;
    let pages = document.pages();
    let page_count = pages.len() as usize;
    if page_count == 0 {
        return Err("This PDF has no pages".into());
    }
    let index = page.min(page_count - 1);
    let pdf_page = pages.get(index as i32).map_err(|error| error.to_string())?;
    let page_size_pts = egui::vec2(pdf_page.width().value, pdf_page.height().value);
    let config = PdfRenderConfig::new()
        .scale_page_by_factor(scale)
        .set_fixed_size(width as i32, height as i32)
        .set_origin(
            -(origin_pts.x * scale).round() as i32,
            -(origin_pts.y * scale).round() as i32,
        );
    let bitmap = pdf_page
        .render_with_config(&config)
        .map_err(|error| error.to_string())?;
    let rendered_width = bitmap.width() as u32;
    let rendered_height = bitmap.height() as u32;
    Ok(RenderedPage {
        page_count,
        page_index: index,
        page_size_pts,
        tile_origin_pts: origin_pts,
        tile_size_pts: egui::vec2(
            rendered_width as f32 / scale,
            rendered_height as f32 / scale,
        ),
        width: rendered_width,
        height: rendered_height,
        rgba: bitmap.as_rgba_bytes(),
    })
}

fn pdfium_instance() -> Result<&'static Pdfium, &'static str> {
    static PDFIUM: OnceLock<Result<Pdfium, String>> = OnceLock::new();
    match PDFIUM.get_or_init(load_pdfium) {
        Ok(pdfium) => Ok(pdfium),
        Err(error) => Err(error.as_str()),
    }
}

fn load_pdfium() -> Result<Pdfium, String> {
    let bindings = bind_next_to_executable()
        .map_or_else(Pdfium::bind_to_system_library, Ok)
        .map_err(|error| {
            format!(
                "Could not load the PDFium library: {error}\n\
                 Run ./scripts/fetch-pdfium.sh to install it."
            )
        })?;
    Ok(Pdfium::new(bindings))
}

fn bind_next_to_executable() -> Option<Box<dyn pdfium_render::prelude::PdfiumLibraryBindings>> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(dir)).ok()
}

fn pick_pdf_file() -> Result<Option<Pdf>, String> {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("PDF", &["pdf"])
        .pick_file()
    else {
        return Ok(None);
    };
    let source_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Document.pdf")
        .to_owned();
    let data = std::fs::read(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    Pdf::new(source_name, data)
        .map(Some)
        .map_err(|error| format!("Could not import {}: {error}", path.display()))
}

impl BlockEditor for PdfEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: true,
        }
    }

    fn direct_editor_intrinsic_size(&mut self, _editors: &mut EditorAccess<'_>) -> Option<Vec2> {
        Some(self.page_size_pts.unwrap_or(DEFAULT_PAGE_SIZE))
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        ui.horizontal_wrapped(|ui| {
            ui.strong("PDF");
            let has_previous = self.page > 0;
            if ui
                .add_enabled(has_previous, egui::Button::new(ICON_ARROW_BACK))
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
                viewport.fit();
            }
        });
        None
    }

    fn direct_editor_has_right_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        ui.heading("PDF");
        if let Some(pdf) = self.block.read() {
            ui.label(pdf.source_name());
        }
        if ui.button("Replace PDF...").clicked() {
            self.replace_from_file();
        }
        if let Some(error) = &self.import_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let page_rect = ui.max_rect();
        let visible_rect = page_rect.intersect(ui.clip_rect());
        let pixels_per_point = ui.ctx().pixels_per_point();
        self.ensure_texture(ui.ctx(), page_rect, visible_rect, pixels_per_point);
        let response = self.paint_texture(ui, page_rect);
        if response.dragged() {
            viewport.pan(response.drag_delta());
        }
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let anchor = ui.ctx().pointer_hover_pos();
                viewport.change_zoom((scroll * 0.002).exp(), anchor);
            }
        }
        None
    }
}
