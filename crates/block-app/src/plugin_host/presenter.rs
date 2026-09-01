use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc, Mutex,
};

use block_plugin_api::{ScreenId, ScreenLayout};
use eframe::{
    egui,
    egui_wgpu::{self, wgpu},
};

use super::backend::{Availability, Frame};

#[cfg(not(target_arch = "wasm32"))]
use super::wasm::{presenter as build_presenter, Presenter as PlatformPresenter};
#[cfg(target_arch = "wasm32")]
use super::web::renderer::{
    presenter as build_presenter, WebSurfacePresenter as PlatformPresenter,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PresenterState {
    Waiting,
    Presenting,
    Unsupported(String),
    Failed(String),
    Released,
}

#[derive(Clone)]
pub(super) struct PresenterStatus(Arc<Mutex<PresenterState>>);

impl PresenterStatus {
    pub(super) fn waiting() -> Self {
        Self(Arc::new(Mutex::new(PresenterState::Waiting)))
    }

    pub(super) fn get(&self) -> PresenterState {
        self.0.lock().unwrap().clone()
    }

    fn set(&self, state: PresenterState) {
        *self.0.lock().unwrap() = state;
    }
}

pub(super) trait SurfacePresenter {
    type Frame;

    fn replace(
        &mut self,
        device: &wgpu::Device,
        regions: &Regions,
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String>;

    fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String>;

    fn paint(
        &self,
        render_pass: &mut wgpu::RenderPass<'static>,
        regions: &Regions,
        surface: u32,
        slot: u32,
    );

    fn release(&mut self, surface: u32);
}

pub(super) const MAX_SURFACES: u32 = 8;
const MAX_PENDING_FRAMES: usize = 8;
const UNPLACED: u32 = u32::MAX;
pub(super) const MAX_REGIONS: u32 = 64;
const MAX_SLOTS: u32 = MAX_SURFACES * MAX_REGIONS;
const REGION_BYTES: u64 = 64;

pub(super) struct Regions {
    buffer: wgpu::Buffer,
    stride: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Quad {
    pub(super) rect: egui::Rect,
    pub(super) corners: [egui::Pos2; 4],
    pub(super) opacity: f32,
}

impl Quad {
    pub(super) fn upright(rect: egui::Rect) -> Self {
        Self {
            rect,
            corners: [
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
            ],
            opacity: 1.0,
        }
    }

    pub(super) fn crop_to(self, clip: egui::Rect) -> Option<(Self, egui::Rect)> {
        let horizontal = self.corners[1] - self.corners[0];
        let vertical = self.corners[3] - self.corners[0];
        let determinant = horizontal.x * vertical.y - horizontal.y * vertical.x;
        if determinant.abs() <= f32::EPSILON {
            return None;
        }
        let to_uv = |point: egui::Pos2| {
            let delta = point - self.corners[0];
            egui::pos2(
                (delta.x * vertical.y - delta.y * vertical.x) / determinant,
                (horizontal.x * delta.y - horizontal.y * delta.x) / determinant,
            )
        };
        let source = egui::Rect::from_points(&[
            to_uv(clip.left_top()),
            to_uv(clip.right_top()),
            to_uv(clip.right_bottom()),
            to_uv(clip.left_bottom()),
        ])
        .intersect(egui::Rect::from_min_max(
            egui::Pos2::ZERO,
            egui::Pos2::new(1.0, 1.0),
        ));
        if source.width() <= 0.0 || source.height() <= 0.0 {
            return None;
        }
        let point = |u: f32, v: f32| self.corners[0] + horizontal * u + vertical * v;
        let corners = [
            point(source.min.x, source.min.y),
            point(source.max.x, source.min.y),
            point(source.max.x, source.max.y),
            point(source.min.x, source.max.y),
        ];
        Some((
            Self {
                rect: egui::Rect::from_points(&corners),
                corners,
                opacity: self.opacity,
            },
            source,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Region {
    pub(super) surface: u32,
    pub(super) slot: u32,
    pub(super) offset: [f32; 2],
    pub(super) scale: [f32; 2],
    pub(super) quad: Quad,
}

impl Region {
    pub(super) fn of(
        layout: &ScreenLayout,
        surface: u32,
        screen: ScreenId,
        quad: Quad,
        source: egui::Rect,
        slot: u32,
    ) -> Option<Self> {
        if layout.is_empty() || surface >= MAX_SURFACES || slot >= MAX_REGIONS {
            return None;
        }
        let placement = layout.placement(screen)?;
        let width = layout.width as f32;
        let height = layout.height as f32;
        let left = placement.x as f32 + placement.width as f32 * source.min.x;
        let top = placement.y as f32 + placement.height as f32 * source.min.y;
        Some(Self {
            surface,
            slot: surface * MAX_REGIONS + slot,
            offset: [left / width, top / height],
            scale: [
                placement.width as f32 * source.width() / width,
                placement.height as f32 * source.height() / height,
            ],
            quad,
        })
    }
}

impl Regions {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let stride = device
            .limits()
            .min_uniform_buffer_offset_alignment
            .max(REGION_BYTES as u32);
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("plugin surface regions"),
                size: u64::from(stride) * u64::from(MAX_SLOTS),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            stride,
        }
    }

    pub(super) fn layout_entry() -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(REGION_BYTES),
            },
            count: None,
        }
    }

    pub(super) fn binding(&self) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: &self.buffer,
            offset: 0,
            size: wgpu::BufferSize::new(REGION_BYTES),
        })
    }

    pub(super) fn write(
        &self,
        queue: &wgpu::Queue,
        region: &Region,
        screen: &egui_wgpu::ScreenDescriptor,
    ) {
        let slot = region.slot.min(MAX_SLOTS - 1);
        let scale = screen.pixels_per_point;
        let width = screen.size_in_pixels[0] as f32;
        let height = screen.size_in_pixels[1] as f32;
        let rect = region.quad.rect;
        let left = (scale * rect.min.x).round().clamp(0.0, width);
        let right = (scale * rect.max.x).round().clamp(left, width);
        let top = (scale * rect.min.y).round().clamp(0.0, height);
        let bottom = (scale * rect.max.y).round().clamp(top, height);
        let viewport = egui::vec2((right - left).max(1.0), (bottom - top).max(1.0));
        let mut values = [0.0; 16];
        values[..2].copy_from_slice(&region.offset);
        values[2..4].copy_from_slice(&region.scale);
        for (index, corner) in region.quad.corners.iter().enumerate() {
            values[4 + index * 2] = (corner.x * scale - left) / viewport.x * 2.0 - 1.0;
            values[5 + index * 2] = 1.0 - (corner.y * scale - top) / viewport.y * 2.0;
        }
        values[12] = region.quad.opacity.clamp(0.0, 1.0);
        queue.write_buffer(
            &self.buffer,
            u64::from(self.stride) * u64::from(slot),
            bytemuck::cast_slice(&values),
        );
    }

    pub(super) fn offset(&self, slot: u32) -> u32 {
        self.stride * slot.min(MAX_SLOTS - 1)
    }
}

#[derive(Default)]
pub(super) struct Shared {
    pub(super) layout: ScreenLayout,
    pub(super) frames: Vec<Frame>,
}

impl Shared {
    pub(super) fn publish(&mut self, layout: &ScreenLayout, frame: Option<Frame>) {
        self.layout.clone_from(layout);
        let Some(frame) = frame else {
            return;
        };
        if self.frames.len() >= MAX_PENDING_FRAMES {
            self.frames.remove(0);
        }
        self.frames.push(frame);
    }
}

pub(super) enum PresenterCommand {
    Present {
        shared: Arc<Mutex<Shared>>,
        screen: ScreenId,
        quad: Quad,
        source: egui::Rect,
        slot: u32,
    },
    Release,
}

pub(super) struct PresenterCallback {
    command: PresenterCommand,
    status: PresenterStatus,
    surface: u32,
    slot: AtomicU32,
}

impl PresenterCallback {
    pub(super) fn present(
        surface: u32,
        status: PresenterStatus,
        shared: Arc<Mutex<Shared>>,
        screen: ScreenId,
        quad: Quad,
        source: egui::Rect,
        slot: u32,
    ) -> Self {
        Self {
            command: PresenterCommand::Present {
                shared,
                screen,
                quad,
                source,
                slot,
            },
            status,
            surface,
            slot: AtomicU32::new(UNPLACED),
        }
    }

    pub(super) fn release(surface: u32, status: PresenterStatus) -> Self {
        Self {
            command: PresenterCommand::Release,
            status,
            surface,
            slot: AtomicU32::new(UNPLACED),
        }
    }
}

impl egui_wgpu::CallbackTrait for PresenterCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(presenter) = resources.get_mut::<Presenter>() else {
            self.status.set(PresenterState::Unsupported(
                "The active renderer has no plugin surface presenter.".to_owned(),
            ));
            return Vec::new();
        };
        match &self.command {
            PresenterCommand::Present {
                shared,
                screen,
                quad,
                source,
                slot,
            } => {
                let (frames, region) = {
                    let mut shared = shared.lock().unwrap();
                    let frames = std::mem::take(&mut shared.frames);
                    (
                        frames,
                        Region::of(&shared.layout, self.surface, *screen, *quad, *source, *slot),
                    )
                };
                let mut failure = None;
                for frame in &frames {
                    let applied = presenter
                        .replace(device, self.surface, frame)
                        .and_then(|()| presenter.prepare(queue, self.surface, frame));
                    if let Err(error) = applied {
                        failure = Some(error);
                    }
                }
                match failure {
                    Some(error) => self.status.set(PresenterState::Failed(error)),
                    None => self.status.set(PresenterState::Presenting),
                }
                match region {
                    Some(region) => {
                        presenter.regions.write(queue, &region, screen_descriptor);
                        self.slot.store(region.slot, Ordering::Relaxed);
                    }
                    None => self.slot.store(UNPLACED, Ordering::Relaxed),
                }
            }
            PresenterCommand::Release => {
                presenter.release(self.surface);
                self.status.set(PresenterState::Released);
            }
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let slot = self.slot.load(Ordering::Relaxed);
        if slot == UNPLACED {
            return;
        }
        if let Some(presenter) = resources.get::<Presenter>() {
            presenter.paint(render_pass, self.surface, slot);
        }
    }
}

pub(super) struct Presenter {
    regions: Regions,
    platform: Option<PlatformPresenter>,
}

pub(super) fn install(creation_context: &eframe::CreationContext<'_>) -> Availability {
    let Some(render_state) = creation_context.wgpu_render_state.as_ref() else {
        return Availability::missing();
    };
    let platform = build_presenter(render_state);
    let availability = Availability(platform.as_ref().map(|_| ()).map_err(Clone::clone));
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(Presenter {
            regions: Regions::new(&render_state.device),
            platform: platform.ok(),
        });
    availability
}

impl Presenter {
    fn replace(
        &mut self,
        device: &wgpu::Device,
        surface: u32,
        frame: &Frame,
    ) -> Result<(), String> {
        match &mut self.platform {
            Some(presenter) => presenter.replace(device, &self.regions, surface, frame),
            None => Err(UNSUPPORTED.to_owned()),
        }
    }

    fn prepare(&mut self, queue: &wgpu::Queue, surface: u32, frame: &Frame) -> Result<(), String> {
        match &mut self.platform {
            Some(presenter) => presenter.prepare(queue, surface, frame),
            None => Err(UNSUPPORTED.to_owned()),
        }
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>, surface: u32, slot: u32) {
        if let Some(presenter) = &self.platform {
            presenter.paint(render_pass, &self.regions, surface, slot);
        }
    }

    fn release(&mut self, surface: u32) {
        if let Some(presenter) = &mut self.platform {
            presenter.release(surface);
        }
    }
}

const UNSUPPORTED: &str = "This build has no presenter for that plugin surface.";
