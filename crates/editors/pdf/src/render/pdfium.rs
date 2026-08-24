use std::{
    sync::{mpsc, OnceLock},
    thread,
    time::Instant,
};

use block_editor_plugin::{
    egui::{self, Pos2, Rect},
    PerformanceReporter, Waker,
};
use pdfium_render::prelude::{PdfBitmap, PdfBitmapFormat, PdfRenderConfig, Pdfium};

use super::{
    RenderJob, RenderJobMessage, RenderJobResult, RenderTarget, RenderedTile, DETAIL_MAX_DIM,
    MIN_SCALE,
};

const MAX_PAGE_DIM: f32 = 100_000.0;

pub(crate) fn spawn_render_job(
    data: Vec<u8>,
    page: usize,
    target: RenderTarget,
    waker: Waker,
    performance: PerformanceReporter,
) -> RenderJob {
    let requested_at = Instant::now();
    let channel_started = Instant::now();
    let (sender, receiver) = mpsc::channel();
    performance.record_duration("Channel creation", channel_started.elapsed());
    let spawn_started = Instant::now();
    let worker_performance = performance.clone();
    thread::Builder::new()
        .name("pdf-render".into())
        .spawn(move || {
            let thread_started = Instant::now();
            worker_performance
                .record_duration("Thread start", thread_started.duration_since(requested_at));
            let result = render_tile(&data, page, target, &worker_performance);
            let completed_at = Instant::now();
            worker_performance
                .record_duration("Worker total", completed_at.duration_since(thread_started));
            let send_started = Instant::now();
            let _ = sender.send(RenderJobMessage {
                completed_at,
                result,
            });
            worker_performance.record_duration("Result send", send_started.elapsed());
            let wake_started = Instant::now();
            waker.wake();
            worker_performance.record_duration("Wake", wake_started.elapsed());
        })
        .expect("failed to start pdf render job");
    performance.record_duration("Thread spawn", spawn_started.elapsed());
    RenderJob { receiver }
}

fn render_tile(
    data: &[u8],
    page: usize,
    target: RenderTarget,
    performance: &PerformanceReporter,
) -> RenderJobResult {
    let phase = Instant::now();
    let pdfium = pdfium_instance().map_err(str::to_owned)?;
    performance.record_duration("PDFium instance", phase.elapsed());

    let phase = Instant::now();
    let document = pdfium
        .load_pdf_from_byte_slice(data, None)
        .map_err(|error| error.to_string())?;
    performance.record_duration("Document load", phase.elapsed());

    let phase = Instant::now();
    let pages = document.pages();
    let page_count = pages.len() as usize;
    if page_count == 0 {
        return Err("This PDF has no pages".into());
    }
    let index = page.min(page_count - 1);
    let pdf_page = pages.get(index as i32).map_err(|error| error.to_string())?;
    let page_size_pts = egui::vec2(pdf_page.width().value, pdf_page.height().value);
    performance.record_duration("Page setup", phase.elapsed());
    let phase = Instant::now();

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
    performance.record_duration("Target setup", phase.elapsed());
    performance.record_count("Output width", width as u64);
    performance.record_count("Output height", height as u64);

    let phase = Instant::now();

    let config = PdfRenderConfig::new()
        .set_fixed_size(page_width, page_height)
        .set_origin(-origin_px.x as i32, -origin_px.y as i32);
    let mut bitmap = PdfBitmap::empty(width, height, PdfBitmapFormat::default())
        .map_err(|error| error.to_string())?;
    performance.record_duration("Bitmap allocation", phase.elapsed());
    let phase = Instant::now();
    pdf_page
        .render_into_bitmap_with_config(&mut bitmap, &config)
        .map_err(|error| error.to_string())?;
    performance.record_duration("PDFium render", phase.elapsed());
    let phase = Instant::now();
    let rgba = bitmap.as_rgba_bytes();
    performance.record_duration("RGBA copy", phase.elapsed());
    let phase = Instant::now();
    let image = egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba);
    performance.record_duration("Color image", phase.elapsed());
    performance.record_count("Pixels", width as u64 * height as u64);
    Ok(RenderedTile {
        page_count,
        page_index: index,
        page_size_pts,
        scale,
        origin_pts: Pos2::new(origin_px.x / scale, origin_px.y / scale),
        size_pts: egui::vec2(width as f32 / scale, height as f32 / scale),
        image,
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
