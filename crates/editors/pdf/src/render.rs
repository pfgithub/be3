use std::sync::mpsc::Receiver;

use block_editor_plugin::egui::{Pos2, Vec2};

#[cfg(not(target_arch = "wasm32"))]
mod pdfium;
#[cfg(target_arch = "wasm32")]
mod unsupported;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use pdfium::spawn_render_job;
#[cfg(target_arch = "wasm32")]
pub(crate) use unsupported::spawn_render_job;

pub(crate) const DETAIL_MAX_DIM: f32 = 4096.0;
pub(crate) const MIN_SCALE: f32 = 0.01;

pub(crate) struct RenderedTile {
    pub(crate) page_count: usize,
    pub(crate) page_index: usize,
    pub(crate) page_size_pts: Vec2,
    pub(crate) scale: f32,
    pub(crate) origin_pts: Pos2,
    pub(crate) size_pts: Vec2,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

#[derive(Clone, Copy)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) enum RenderTarget {
    FullPage {
        max_width: f32,
        max_height: f32,
    },
    Region {
        scale: f32,
        origin_pts: Pos2,
        size_pts: Vec2,
    },
}

#[derive(Clone, Copy)]
pub(crate) struct RenderRequest {
    pub(crate) revision: u64,
    pub(crate) page: usize,
    pub(crate) target: RenderTarget,
}

pub(crate) type RenderJobResult = Result<RenderedTile, String>;

pub(crate) type RenderJob = Receiver<RenderJobResult>;
