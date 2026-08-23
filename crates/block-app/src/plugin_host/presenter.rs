#![cfg_attr(
    not(any(target_arch = "wasm32", target_os = "windows", target_os = "linux")),
    allow(dead_code)
)]

use std::sync::{Arc, Mutex};

use eframe::{
    egui,
    egui_wgpu::{self, wgpu},
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
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String>;

    fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String>;

    fn regions(&self) -> &Regions;

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>, surface: u32, slot: u32);

    fn release(&mut self, surface: u32);
}

pub(super) const MAX_SURFACES: u32 = 8;
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
}

impl Default for Quad {
    fn default() -> Self {
        Self::upright(egui::Rect::ZERO)
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

impl Default for Region {
    fn default() -> Self {
        Self {
            surface: 0,
            slot: 0,
            offset: [0.0, 0.0],
            scale: [1.0, 1.0],
            quad: Quad::default(),
        }
    }
}

impl Region {
    pub(super) fn of(
        layout: &block_plugin_api::ScreenLayout,
        surface: u32,
        screen: block_plugin_api::ScreenId,
        quad: Quad,
    ) -> Option<Self> {
        if layout.is_empty() || surface >= MAX_SURFACES {
            return None;
        }
        let index = layout
            .screens
            .iter()
            .position(|placement| placement.screen == screen)?;
        if index >= MAX_REGIONS as usize {
            return None;
        }
        let slot = surface * MAX_REGIONS + index as u32;
        let placement = &layout.screens[index];
        let width = layout.width as f32;
        let height = layout.height as f32;
        Some(Self {
            surface,
            slot,
            offset: [placement.x as f32 / width, placement.y as f32 / height],
            scale: [
                placement.width as f32 / width,
                placement.height as f32 / height,
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

pub(super) enum PresenterCommand<Frame> {
    Present(Frame),
    Release,
}

pub(super) struct PresenterCallback<Frame> {
    pub(super) command: PresenterCommand<Frame>,
    pub(super) status: PresenterStatus,
    pub(super) region: Region,
}

impl<Frame> egui_wgpu::CallbackTrait for PresenterCallback<Frame>
where
    Frame: Send + Sync + 'static,
    Presenter: SurfacePresenter<Frame = Frame>,
{
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
            PresenterCommand::Present(frame) => {
                presenter
                    .regions()
                    .write(queue, &self.region, screen_descriptor);
                if let Err(error) = presenter.replace(device, self.region.surface, frame) {
                    self.status.set(PresenterState::Failed(error));
                } else if let Err(error) = presenter.prepare(queue, self.region.surface, frame) {
                    self.status.set(PresenterState::Failed(error));
                } else {
                    self.status.set(PresenterState::Presenting);
                }
            }
            PresenterCommand::Release => {
                presenter.release(self.region.surface);
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
        if matches!(self.command, PresenterCommand::Present(_)) {
            if let Some(presenter) = resources.get::<Presenter>() {
                presenter.paint(render_pass, self.region.surface, self.region.slot);
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub(super) use super::linux::LinuxSurfacePresenter as Presenter;
#[cfg(not(any(target_arch = "wasm32", target_os = "windows", target_os = "linux")))]
pub(super) use super::unavailable::UnavailablePresenter as Presenter;
#[cfg(target_arch = "wasm32")]
pub(super) use super::web::renderer::WebSurfacePresenter as Presenter;
#[cfg(target_os = "windows")]
pub(super) use super::windows::WindowsSurfacePresenter as Presenter;
