use std::{sync::mpsc::TryRecvError, time::Instant};

use block_editor_plugin::{
    egui::{self, Color32, Pos2, Rect, Vec2},
    Waker,
};

use crate::render::{
    spawn_render_job, RenderJob, RenderRequest, RenderTarget, RenderedTile, DETAIL_MAX_DIM,
    MIN_SCALE,
};

const TILE_MIN_SCALE_FACTOR: f32 = 0.4;
const TILE_MAX_SCALE_FACTOR: f32 = 1.15;

pub(crate) struct PageFacts {
    pub(crate) page_count: usize,
    pub(crate) page_index: usize,
    pub(crate) page_size_pts: Vec2,
}

struct Tile {
    revision: u64,
    page: usize,
    scale: f32,
    origin_pts: Pos2,
    size_pts: Vec2,
    texture: egui::TextureHandle,
}

impl Tile {
    fn rect(&self) -> Rect {
        Rect::from_min_size(self.origin_pts, self.size_pts)
    }

    fn is_stale(&self, revision: u64, page: usize) -> bool {
        self.revision != revision || self.page != page
    }
}

#[derive(Default)]
pub(crate) struct Pane {
    base: Option<Tile>,
    detail: Option<Tile>,
    job: Option<(RenderRequest, RenderJob)>,
    failed: Option<(u64, usize)>,
    error: Option<String>,
}

impl Pane {
    pub(crate) fn poll(&mut self, context: &egui::Context) -> Option<PageFacts> {
        let (request, job) = self.job.as_ref()?;
        let request = *request;
        let job_id = job.id;
        let receive_started = Instant::now();
        let received = job.try_recv();
        let receive_elapsed = receive_started.elapsed();
        match received {
            Ok(message) => {
                eprintln!(
                    "pdf timing job={job_id} ready_to_poll={:?} try_receive={receive_elapsed:?}",
                    message.completed_at.elapsed()
                );
                match message.result {
                    Ok(rendered) => {
                        self.job = None;
                        self.error = None;
                        self.failed = None;
                        let facts = PageFacts {
                            page_count: rendered.page_count,
                            page_index: rendered.page_index,
                            page_size_pts: rendered.page_size_pts,
                        };
                        self.store(context, job_id, request, rendered);
                        Some(facts)
                    }
                    Err(error) => {
                        self.job = None;
                        self.error = Some(error);
                        self.failed = Some((request.revision, request.page));
                        None
                    }
                }
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.job = None;
                None
            }
        }
    }

    fn store(
        &mut self,
        context: &egui::Context,
        job_id: u64,
        request: RenderRequest,
        rendered: RenderedTile,
    ) {
        let (slot, name) = match request.target {
            RenderTarget::FullPage { .. } => (&mut self.base, "pdf-page"),
            RenderTarget::Region { .. } => (&mut self.detail, "pdf-detail"),
        };
        let image = rendered.image;
        let texture_started = Instant::now();
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
                    texture: context.load_texture(name, image, egui::TextureOptions::LINEAR),
                });
            }
        }
        eprintln!(
            "pdf timing job={job_id} texture_delta={:?}",
            texture_started.elapsed()
        );
    }

    pub(crate) fn ensure(
        &mut self,
        revision: u64,
        page: usize,
        page_size_pts: Option<Vec2>,
        page_rect: Rect,
        visible_rect: Rect,
        pixels_per_point: f32,
        waker: Waker,
        data: impl FnOnce() -> Option<Vec<u8>>,
    ) {
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
        if self.job.is_some() || self.failed == Some((revision, page)) {
            return;
        }
        let target = if self.base.is_none() {
            if visible_rect.width() <= 0.0 || visible_rect.height() <= 0.0 {
                return;
            }
            RenderTarget::FullPage {
                max_width: visible_rect.width() * pixels_per_point,
                max_height: visible_rect.height() * pixels_per_point,
            }
        } else {
            match self.detail_target(page_size_pts, page_rect, visible_rect, pixels_per_point) {
                Some(target) => target,
                None => return,
            }
        };
        let data_started = Instant::now();
        let Some(data) = data() else {
            return;
        };
        eprintln!(
            "pdf timing revision={revision} page={page} data_copy={:?} bytes={}",
            data_started.elapsed(),
            data.len()
        );
        let request = RenderRequest {
            revision,
            page,
            target,
        };
        let spawn_started = Instant::now();
        let job = spawn_render_job(data, page, target, waker);
        eprintln!(
            "pdf timing job={} spawn_render_job_call={:?}",
            job.id,
            spawn_started.elapsed()
        );
        self.job = Some((request, job));
    }

    fn detail_target(
        &self,
        page_size_pts: Option<Vec2>,
        page_rect: Rect,
        visible_rect: Rect,
        pixels_per_point: f32,
    ) -> Option<RenderTarget> {
        let page_size = page_size_pts?;
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

        let region = tile_region(visible, bounds);
        Some(RenderTarget::Region {
            scale,
            origin_pts: region.min,
            size_pts: region.size(),
        })
    }

    pub(crate) fn paint(&self, painter: &egui::Painter, page_rect: Rect, page_size: Vec2) {
        let points_per_pdf_point = page_rect.width() / page_size.x.max(f32::EPSILON);
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
    }

    pub(crate) fn has_page(&self) -> bool {
        self.base.is_some() || self.detail.is_some()
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

fn page_position(position: Pos2, page_rect: Rect, points_per_pdf_point: f32) -> Pos2 {
    let inverse = 1.0 / points_per_pdf_point.max(f32::EPSILON);
    Pos2::new(
        (position.x - page_rect.min.x) * inverse,
        (position.y - page_rect.min.y) * inverse,
    )
}

fn tile_region(visible: Rect, bounds: Rect) -> Rect {
    visible.intersect(bounds)
}

#[cfg(test)]
mod tests;
