use std::sync::{Arc, Mutex};

use eframe::egui_wgpu::{self, wgpu};

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

    fn replace(&mut self, device: &wgpu::Device, frame: &Self::Frame) -> Result<(), String>;

    fn prepare(&mut self, queue: &wgpu::Queue, frame: &Self::Frame) -> Result<(), String>;

    fn regions(&self) -> &Regions;

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>, slot: u32);

    fn release(&mut self);
}

pub(super) const MAX_REGIONS: u32 = 64;

/// Uniform storage for the atlas sub-rectangle each plugin editor paints. One
/// slot per screen, addressed through a dynamic bind group offset so several
/// editors can blit different parts of the same plugin surface in one frame.
pub(super) struct Regions {
    buffer: wgpu::Buffer,
    stride: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Region {
    pub(super) slot: u32,
    pub(super) offset: [f32; 2],
    pub(super) scale: [f32; 2],
}

impl Default for Region {
    fn default() -> Self {
        Self {
            slot: 0,
            offset: [0.0, 0.0],
            scale: [1.0, 1.0],
        }
    }
}

impl Region {
    pub(super) fn of(
        layout: &block_plugin_api::ScreenLayout,
        screen: block_plugin_api::ScreenId,
    ) -> Option<Self> {
        if layout.is_empty() {
            return None;
        }
        let slot = layout
            .screens
            .iter()
            .position(|placement| placement.screen == screen)?;
        if slot >= MAX_REGIONS as usize {
            return None;
        }
        let placement = &layout.screens[slot];
        let width = layout.width as f32;
        let height = layout.height as f32;
        Some(Self {
            slot: slot as u32,
            offset: [placement.x as f32 / width, placement.y as f32 / height],
            scale: [
                placement.width as f32 / width,
                placement.height as f32 / height,
            ],
        })
    }
}

impl Regions {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let stride = device
            .limits()
            .min_uniform_buffer_offset_alignment
            .max(std::mem::size_of::<[f32; 4]>() as u32);
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("plugin surface regions"),
                size: u64::from(stride) * u64::from(MAX_REGIONS),
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
                min_binding_size: wgpu::BufferSize::new(16),
            },
            count: None,
        }
    }

    pub(super) fn binding(&self) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: &self.buffer,
            offset: 0,
            size: wgpu::BufferSize::new(16),
        })
    }

    pub(super) fn write(&self, queue: &wgpu::Queue, region: &Region) {
        let slot = region.slot.min(MAX_REGIONS - 1);
        let values = [
            region.offset[0],
            region.offset[1],
            region.scale[0],
            region.scale[1],
        ];
        queue.write_buffer(
            &self.buffer,
            u64::from(self.stride) * u64::from(slot),
            bytemuck::cast_slice(&values),
        );
    }

    pub(super) fn offset(&self, slot: u32) -> u32 {
        self.stride * slot.min(MAX_REGIONS - 1)
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
    WebSurfacePresenter: SurfacePresenter<Frame = Frame>,
{
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(presenter) = resources.get_mut::<WebSurfacePresenter>() else {
            self.status.set(PresenterState::Unsupported(
                "The active renderer has no plugin surface presenter.".to_owned(),
            ));
            return Vec::new();
        };
        match &self.command {
            PresenterCommand::Present(frame) => {
                presenter.regions().write(queue, &self.region);
                if let Err(error) = presenter.replace(device, frame) {
                    self.status.set(PresenterState::Failed(error));
                } else if let Err(error) = presenter.prepare(queue, frame) {
                    self.status.set(PresenterState::Failed(error));
                } else {
                    self.status.set(PresenterState::Presenting);
                }
            }
            PresenterCommand::Release => {
                presenter.release();
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
            if let Some(presenter) = resources.get::<WebSurfacePresenter>() {
                presenter.paint(render_pass, self.region.slot);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) use super::web::renderer::WebSurfacePresenter;
#[cfg(target_os = "windows")]
pub(super) use super::windows::WindowsSurfacePresenter as WebSurfacePresenter;
