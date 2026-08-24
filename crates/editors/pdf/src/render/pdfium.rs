use std::sync::{mpsc, OnceLock};
use std::thread;

use block_editor_plugin::{
    egui::{self, Pos2, Rect},
    Waker,
};
use pdfium_render::prelude::{PdfBitmap, PdfBitmapFormat, PdfRenderConfig, Pdfium};

use super::{RenderJob, RenderJobResult, RenderTarget, RenderedTile, DETAIL_MAX_DIM, MIN_SCALE};

const MAX_PAGE_DIM: f32 = 100_000.0;

pub(crate) fn spawn_render_job(
    data: Vec<u8>,
    page: usize,
    target: RenderTarget,
    waker: Waker,
) -> RenderJob {
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("pdf-render".into())
        .spawn(move || {
            let _ = sender.send(render_tile(&data, page, target));
            waker.wake();
        })
        .expect("failed to start pdf render job");
    receiver
}

fn render_tile(data: &[u8], page: usize, target: RenderTarget) -> RenderJobResult {
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
    if page_size_pts.x <= 0.0 || page_size_pts.y <= 0.0 {
        return Err("This PDF page is empty".into());
    }
    let bounds = Rect::from_min_size(Pos2::ZERO, page_size_pts);
    let (requested_scale, region) = match target {
        RenderTarget::FullPage {
            max_width,
            max_height,
        } => (
            (max_width.max(1.0) / page_size_pts.x).min(max_height.max(1.0) / page_size_pts.y),
            bounds,
        ),
        RenderTarget::Region {
            scale,
            origin_pts,
            size_pts,
        } => (
            scale,
            Rect::from_min_size(origin_pts, size_pts).intersect(bounds),
        ),
    };
    let scale = requested_scale.clamp(MIN_SCALE, MAX_PAGE_DIM / page_size_pts.max_elem());

    let page_width = ((page_size_pts.x * scale).round() as i32).max(1);
    let page_height = ((page_size_pts.y * scale).round() as i32).max(1);
    let origin_px = egui::vec2(
        (region.min.x * scale).round(),
        (region.min.y * scale).round(),
    );
    let width = ((region.width() * scale).round() as i32).clamp(1, DETAIL_MAX_DIM as i32);
    let height = ((region.height() * scale).round() as i32).clamp(1, DETAIL_MAX_DIM as i32);

    let config = PdfRenderConfig::new()
        .set_fixed_size(page_width, page_height)
        .set_origin(-origin_px.x as i32, -origin_px.y as i32);
    let mut bitmap = PdfBitmap::empty(width, height, PdfBitmapFormat::default())
        .map_err(|error| error.to_string())?;
    pdf_page
        .render_into_bitmap_with_config(&mut bitmap, &config)
        .map_err(|error| error.to_string())?;
    Ok(RenderedTile {
        page_count,
        page_index: index,
        page_size_pts,
        scale,
        origin_pts: Pos2::new(origin_px.x / scale, origin_px.y / scale),
        size_pts: egui::vec2(width as f32 / scale, height as f32 / scale),
        width: width as u32,
        height: height as u32,
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
    let bindings = bind_around_executable()
        .map_or_else(Pdfium::bind_to_system_library, Ok)
        .map_err(|error| format!("Could not load the PDFium library: {error}"))?;
    Ok(Pdfium::new(bindings))
}

fn bind_around_executable() -> Option<Box<dyn pdfium_render::prelude::PdfiumLibraryBindings>> {
    let executable = std::env::current_exe().ok()?;
    let mut directory = executable.parent();
    for _ in 0..3 {
        let current = directory?;
        let name = Pdfium::pdfium_platform_library_name_at_path(current);
        if let Ok(bindings) = Pdfium::bind_to_library(name) {
            return Some(bindings);
        }
        directory = current.parent();
    }
    None
}
