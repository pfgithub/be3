use std::cell::RefCell;

use block_plugin_api::{FrameReady, Message, ScreenLayout};
use eframe::egui_wgpu::wgpu;

use crate::{panes::Panes, screens::Screens, wasm::Attachment};

pub(crate) const SURFACE_KIND: &str = "host texture";

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
    pub(crate) fn new(
        _request_id: u64,
        layout: ScreenLayout,
        generation: u64,
    ) -> Result<Self, String> {
        let gpu = gpu()?;
        configure(&layout);
        Ok(Self {
            panes: Panes::new(FORMAT),
            gpu,
            layout,
            generation,
        })
    }

    pub(crate) fn resize(
        mut self,
        _request_id: u64,
        layout: ScreenLayout,
        generation: u64,
    ) -> Result<Self, String> {
        configure(&layout);
        self.layout = layout;
        self.generation = generation;
        Ok(self)
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    pub(crate) fn descriptor(&self) -> Option<(Message, Vec<Attachment>)> {
        None
    }

    pub(crate) fn set_previews(
        &mut self,
        _layout: &block_plugin_api::PreviewLayout,
    ) -> Result<Option<(Message, Vec<Attachment>)>, String> {
        Ok(None)
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
            None,
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
            buffer: 0,
            damage: Vec::new(),
            synchronization_value: 0,
            repaint_after_micros: painted.repaint.map(|delay| delay.as_micros() as u64),
            attachments: Vec::new(),
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
