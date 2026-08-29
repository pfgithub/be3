use std::cell::RefCell;

use block_plugin_api::{FrameReady, Message, ScreenLayout};
use eframe::egui_wgpu::wgpu;

use crate::{panes::Panes, screens::Screens};

const SCREENS_SURFACE: u32 = 0;

thread_local! {
    static GPU: RefCell<Option<Gpu>> = const { RefCell::new(None) };
}

#[derive(Clone)]
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

pub(crate) fn initialize() -> Result<(), String> {
    let (device, queue) = block_gpu_guest::device_and_queue();
    GPU.with(|gpu| *gpu.borrow_mut() = Some(Gpu { device, queue }));
    Ok(())
}

fn gpu() -> Result<Gpu, String> {
    GPU.with(|gpu| gpu.borrow().clone())
        .ok_or_else(|| "the plugin gpu is not ready".to_owned())
}

pub(crate) struct Surface {
    gpu: Gpu,
    panes: Panes,
    layout: ScreenLayout,
    generation: u64,
}

impl Surface {
    pub(crate) fn new(layout: ScreenLayout, generation: u64) -> Result<Self, String> {
        let gpu = gpu()?;
        configure(&layout);
        Ok(Self {
            panes: Panes::new(FORMAT),
            gpu,
            layout,
            generation,
        })
    }

    pub(crate) fn resize(mut self, layout: ScreenLayout, generation: u64) -> Result<Self, String> {
        configure(&layout);
        self.layout = layout;
        self.generation = generation;
        Ok(self)
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    pub(crate) fn render(
        &mut self,
        screens: &mut Screens,
        phase: f64,
    ) -> Result<Vec<Message>, String> {
        if self.layout.is_empty() {
            return Ok(Vec::new());
        }
        let texture = block_gpu_guest::acquire_surface_texture(SCREENS_SURFACE)?;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let painted = self.panes.paint(
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &view,
            &self.layout,
            screens,
            phase,
        );
        self.gpu
            .queue
            .submit(painted.commands.into_iter().chain([encoder.finish()]));
        block_gpu_guest::present_surface(SCREENS_SURFACE);
        Ok(vec![Message::FrameReady(FrameReady {
            generation: self.generation,
            repaint_after_micros: painted.repaint.map(|delay| delay.as_micros() as u64),
        })])
    }
}

fn configure(layout: &ScreenLayout) {
    if layout.is_empty() {
        return;
    }
    block_gpu_guest::configure_surface(SCREENS_SURFACE, layout.width, layout.height, FORMAT);
}

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
