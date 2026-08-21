#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod pdfium;
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
mod unsupported;

use std::sync::mpsc::{Receiver, TryRecvError};

use block_client::{
    blocks::pdf::{Pdf, PdfOperation},
    BlockClient, BlockHandle,
};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, TextureHandle, Vec2};
use egui_material_icons::{
    icons::{ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_PICTURE_AS_PDF},
    MaterialIcon,
};

use super::{
    BlockEditor, ConfigurableEditor, CreationOptions, DirectEditorCapabilities,
    DirectEditorViewport, EditorAccess, EditorAction, EditorKind,
};
use crate::platform::{FileFilter, FilePicker, PickedFile};
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
use pdfium::spawn_render_job;
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
use unsupported::spawn_render_job;

const PDF_FILTER: FileFilter = FileFilter {
    name: "PDF",
    default_file_name: "Document.pdf",
    extensions: &["pdf"],
    mime_types: &["application/pdf"],
};

const DEFAULT_PAGE_SIZE: Vec2 = egui::vec2(612.0, 792.0);
const DETAIL_MAX_DIM: f32 = 4096.0;
const MIN_SCALE: f32 = 0.01;
const TILE_MARGIN_FACTOR: f32 = 0.35;
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
    picker: FilePicker,
    pdf: Option<Pdf>,
    error: Option<String>,
}

impl CreationOptions for ChosenPdf {
    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        match self.picker.poll(ui.ctx()).map(|file| file.and_then(decode)) {
            Some(Ok(pdf)) => {
                self.pdf = Some(pdf);
                self.error = None;
            }
            Some(Err(error)) => {
                self.pdf = None;
                self.error = Some(error);
            }
            None => {}
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.picker.is_open(), egui::Button::new("Choose file..."))
                .clicked()
            {
                self.picker.open(ui.ctx(), &PDF_FILTER);
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

struct RenderedTile {
    page_count: usize,
    page_index: usize,
    page_size_pts: Vec2,
    scale: f32,
    origin_pts: Pos2,
    size_pts: Vec2,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

#[derive(Clone, Copy)]
#[cfg_attr(any(target_os = "android", target_arch = "wasm32"), allow(dead_code))]
enum RenderTarget {
    FullPage,
    Region {
        scale: f32,
        origin_pts: Pos2,
        size_pts: Vec2,
    },
}

#[derive(Clone, Copy)]
struct RenderRequest {
    revision: u64,
    page: usize,
    target: RenderTarget,
}

struct Tile {
    revision: u64,
    page: usize,
    scale: f32,
    origin_pts: Pos2,
    size_pts: Vec2,
    texture: TextureHandle,
}

impl Tile {
    fn rect(&self) -> Rect {
        Rect::from_min_size(self.origin_pts, self.size_pts)
    }

    fn is_stale(&self, revision: u64, page: usize) -> bool {
        self.revision != revision || self.page != page
    }
}

type RenderJobResult = Result<RenderedTile, String>;

pub(crate) struct PdfEditor {
    block: BlockHandle<Pdf>,
    page: usize,
    page_count: Option<usize>,
    page_size_pts: Option<Vec2>,
    base: Option<Tile>,
    detail: Option<Tile>,
    render_job: Option<(RenderRequest, Receiver<RenderJobResult>)>,
    failed: Option<(u64, usize)>,
    render_error: Option<String>,
    picker: FilePicker,
    import_error: Option<String>,
}

impl PdfEditor {
    pub(crate) fn new(block: BlockHandle<Pdf>) -> Self {
        Self {
            block,
            page: 0,
            page_count: None,
            page_size_pts: None,
            base: None,
            detail: None,
            render_job: None,
            failed: None,
            render_error: None,
            picker: FilePicker::default(),
            import_error: None,
        }
    }

    fn page_size(&self) -> Vec2 {
        self.page_size_pts.unwrap_or(DEFAULT_PAGE_SIZE)
    }

    /// Where the page itself sits inside the area the host gave us, keeping the
    /// page's own aspect ratio however the content rect is shaped.
    fn page_rect(&self, content_rect: Rect) -> Rect {
        let page_size = self.page_size();
        let scale = (content_rect.width() / page_size.x)
            .min(content_rect.height() / page_size.y)
            .max(f32::EPSILON);
        Rect::from_center_size(content_rect.center(), page_size * scale)
    }

    fn poll_render(&mut self, context: &egui::Context) {
        let Some((request, receiver)) = &self.render_job else {
            return;
        };
        let request = *request;
        match receiver.try_recv() {
            Ok(Ok(rendered)) => {
                self.render_job = None;
                self.render_error = None;
                self.failed = None;
                self.page_count = Some(rendered.page_count);
                if self.page >= rendered.page_count {
                    self.page = rendered.page_index;
                }
                self.page_size_pts = Some(rendered.page_size_pts);
                let slot = match request.target {
                    RenderTarget::FullPage => &mut self.base,
                    RenderTarget::Region { .. } => &mut self.detail,
                };
                let name = match request.target {
                    RenderTarget::FullPage => format!("pdf-page-{}", self.block.id()),
                    RenderTarget::Region { .. } => format!("pdf-detail-{}", self.block.id()),
                };
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [rendered.width as usize, rendered.height as usize],
                    &rendered.rgba,
                );
                match slot {
                    Some(tile) => {
                        tile.texture.set(image, egui::TextureOptions::LINEAR);
                        tile.revision = request.revision;
                        tile.page = rendered.page_index;
                        tile.scale = rendered.scale;
                        tile.origin_pts = rendered.origin_pts;
                        tile.size_pts = rendered.size_pts;
                    }
                    None => {
                        *slot = Some(Tile {
                            revision: request.revision,
                            page: rendered.page_index,
                            scale: rendered.scale,
                            origin_pts: rendered.origin_pts,
                            size_pts: rendered.size_pts,
                            texture: context.load_texture(
                                name,
                                image,
                                egui::TextureOptions::LINEAR,
                            ),
                        });
                    }
                }
            }
            Ok(Err(error)) => {
                self.render_job = None;
                self.render_error = Some(error);
                self.failed = Some((request.revision, request.page));
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => self.render_job = None,
        }
    }

    fn ensure_textures(
        &mut self,
        context: &egui::Context,
        page_rect: Rect,
        visible_rect: Rect,
        pixels_per_point: f32,
    ) {
        self.poll_render(context);
        if self.block.read().is_none() {
            return;
        }
        let revision = self.block.revision();
        let page = self.page;
        if self
            .base
            .as_ref()
            .is_some_and(|tile| tile.is_stale(revision, page))
        {
            self.base = None;
        }
        if self
            .detail
            .as_ref()
            .is_some_and(|tile| tile.is_stale(revision, page))
        {
            self.detail = None;
        }
        if self.render_job.is_some() || self.failed == Some((revision, page)) {
            return;
        }

        let target = if self.base.is_none() {
            RenderTarget::FullPage
        } else {
            match self.detail_target(page_rect, visible_rect, pixels_per_point) {
                Some(target) => target,
                None => return,
            }
        };
        self.spawn_render(
            context,
            RenderRequest {
                revision,
                page,
                target,
            },
        );
    }

    /// The slice of the page worth rendering at full resolution, or `None` when
    /// the full-page render already carries enough detail for this zoom level.
    fn detail_target(
        &self,
        page_rect: Rect,
        visible_rect: Rect,
        pixels_per_point: f32,
    ) -> Option<RenderTarget> {
        let page_size = self.page_size_pts?;
        if page_rect.width() <= 0.0 || visible_rect.width() <= 0.0 || visible_rect.height() <= 0.0 {
            return None;
        }
        let points_per_pdf_point = page_rect.width() / page_size.x;
        let bounds = Rect::from_min_size(Pos2::ZERO, page_size);
        let visible = Rect::from_min_max(
            page_position(visible_rect.min, page_rect, points_per_pdf_point),
            page_position(visible_rect.max, page_rect, points_per_pdf_point),
        )
        .intersect(bounds);
        if visible.width() <= 0.0 || visible.height() <= 0.0 {
            return None;
        }

        let scale = (points_per_pdf_point * pixels_per_point)
            .min(DETAIL_MAX_DIM / visible.width())
            .min(DETAIL_MAX_DIM / visible.height())
            .max(MIN_SCALE);
        let base_scale = self.base.as_ref().map_or(0.0, |base| base.scale);
        if scale <= base_scale * TILE_MAX_SCALE_FACTOR {
            return None;
        }
        if self.detail.as_ref().is_some_and(|detail| {
            scale <= detail.scale * TILE_MAX_SCALE_FACTOR
                && scale >= detail.scale * TILE_MIN_SCALE_FACTOR
                && detail.rect().expand(0.5).contains_rect(visible)
        }) {
            return None;
        }

        let region = tile_region(visible, bounds, scale);
        Some(RenderTarget::Region {
            scale,
            origin_pts: region.min,
            size_pts: region.size(),
        })
    }

    fn spawn_render(&mut self, context: &egui::Context, request: RenderRequest) {
        let Some(pdf) = self.block.read() else {
            return;
        };
        let data = pdf.data().to_vec();
        drop(pdf);
        let receiver = spawn_render_job(context, data, request.page, request.target);
        self.render_job = Some((request, receiver));
    }

    fn paint(&self, ui: &mut egui::Ui, page_rect: Rect) -> egui::Response {
        let available = ui.available_size().max(Vec2::splat(1.0));
        let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
        if self.base.is_none() && self.detail.is_none() {
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.centered_and_justified(|ui| {
                    if let Some(error) = &self.render_error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    } else {
                        ui.spinner();
                    }
                });
            });
            return response;
        }

        let points_per_pdf_point = page_rect.width() / self.page_size().x;
        let painter = ui.painter_at(rect);
        painter.rect_filled(page_rect, 0.0, Color32::WHITE);
        for tile in [self.base.as_ref(), self.detail.as_ref()]
            .into_iter()
            .flatten()
        {
            let tile_rect = Rect::from_min_size(
                page_rect.min + tile.origin_pts.to_vec2() * points_per_pdf_point,
                tile.size_pts * points_per_pdf_point,
            );
            painter.image(
                tile.texture.id(),
                tile_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        response
    }

    fn handle_input(&mut self, response: &egui::Response, viewport: &mut DirectEditorViewport) {
        if response.dragged() {
            viewport.pan(response.drag_delta());
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
            viewport.change_zoom(zoom_delta, Some(pointer));
        } else if command && scroll.y != 0.0 {
            viewport.change_zoom((scroll.y * 0.002).exp(), Some(pointer));
        } else if scroll != Vec2::ZERO {
            viewport.pan(scroll);
        }
    }

    fn poll_picker(&mut self, context: &egui::Context) {
        match self.picker.poll(context).map(|file| file.and_then(decode)) {
            Some(Ok(pdf)) => {
                self.block.operate(PdfOperation::Replace { pdf });
                self.page = 0;
                self.import_error = None;
            }
            Some(Err(error)) => self.import_error = Some(error),
            None => {}
        }
    }
}

fn page_position(position: Pos2, page_rect: Rect, points_per_pdf_point: f32) -> Pos2 {
    let inverse = 1.0 / points_per_pdf_point.max(f32::EPSILON);
    Pos2::new(
        (position.x - page_rect.min.x) * inverse,
        (position.y - page_rect.min.y) * inverse,
    )
}

/// Pads the visible slice of the page so small pans stay covered, then trims it
/// back to what one bitmap can hold, keeping the visible slice inside.
fn tile_region(visible: Rect, bounds: Rect, scale: f32) -> Rect {
    let padded = visible
        .expand2(visible.size() * TILE_MARGIN_FACTOR)
        .intersect(bounds);
    let width = padded.width().min(DETAIL_MAX_DIM / scale);
    let height = padded.height().min(DETAIL_MAX_DIM / scale);
    let center = visible.center();
    let x = (center.x - width / 2.0).clamp(padded.min.x, (padded.max.x - width).max(padded.min.x));
    let y =
        (center.y - height / 2.0).clamp(padded.min.y, (padded.max.y - height).max(padded.min.y));
    Rect::from_min_size(Pos2::new(x, y), egui::vec2(width, height))
}

fn decode(file: PickedFile) -> Result<Pdf, String> {
    let PickedFile { name, data } = file;
    Pdf::new(name.clone(), data).map_err(|error| format!("Could not import {name}: {error}"))
}

impl BlockEditor for PdfEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn default_preserve_aspect_ratio(&self) -> bool {
        true
    }

    fn render_aspect_ratio(&self) -> Option<f32> {
        let size = self.page_size_pts?;
        (size.y != 0.0).then(|| size.x / size.y)
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: true,
            supports_pan_and_zoom: true,
        }
    }

    fn direct_editor_fills_viewport(&self) -> bool {
        true
    }

    fn direct_editor_handles_viewport_input(&self, _editors: &EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_intrinsic_size(&mut self, _editors: &mut EditorAccess<'_>) -> Option<Vec2> {
        Some(self.page_size())
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
        self.poll_picker(ui.ctx());
        if ui
            .add_enabled(!self.picker.is_open(), egui::Button::new("Replace PDF..."))
            .clicked()
        {
            self.picker.open(ui.ctx(), &PDF_FILTER);
        }
        if let Some(error) = &self.import_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        None
    }

    /// Inside a canvas the PDF has no viewport of its own, so it just fits the
    /// page into the space it was given and leaves input to the host.
    fn embedded_direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let page_rect = self.page_rect(ui.max_rect());
        let visible_rect = page_rect.intersect(ui.clip_rect());
        let pixels_per_point = ui.ctx().pixels_per_point();
        self.ensure_textures(ui.ctx(), page_rect, visible_rect, pixels_per_point);
        self.paint(ui, page_rect);
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let content_rect = viewport.content_rect().unwrap_or_else(|| ui.max_rect());
        let page_rect = self.page_rect(content_rect);
        let visible_rect = page_rect.intersect(ui.clip_rect());
        let pixels_per_point = ui.ctx().pixels_per_point();
        self.ensure_textures(ui.ctx(), page_rect, visible_rect, pixels_per_point);
        let response = self.paint(ui, page_rect);
        self.handle_input(&response, viewport);
        None
    }
}

#[cfg(test)]
mod tests;
