use block_plugin_api::{FrameReady, Message, ScreenLayout};
use eframe::egui_wgpu::wgpu;
use std::{cell::RefCell, rc::Rc};

use crate::{panes::Panes, screens::Screens, web::Attachment};

pub(crate) const SURFACE_KIND: &str = "canvas";

thread_local! {
    static GPU: RefCell<Option<Rc<Gpu>>> = const { RefCell::new(None) };
}

#[derive(Clone, Debug)]
struct WebDisplay;

impl wgpu::rwh::HasDisplayHandle for WebDisplay {
    fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
        Ok(wgpu::rwh::DisplayHandle::web())
    }
}

struct Gpu {
    canvas: web_sys::OffscreenCanvas,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    format: wgpu::TextureFormat,
    configuration: RefCell<wgpu::SurfaceConfiguration>,
}

impl Gpu {
    fn configure(&self, layout: &ScreenLayout) {
        let width = layout.width.max(1);
        let height = layout.height.max(1);
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        let mut configuration = self.configuration.borrow_mut();
        configuration.width = width;
        configuration.height = height;
        self.surface.configure(&self.device, &configuration);
    }
}

pub(crate) async fn initialize(canvas: web_sys::OffscreenCanvas) -> Result<(), String> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL;
    descriptor.display = Some(Box::new(WebDisplay));
    let instance = wgpu::Instance::new(descriptor);
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas.clone()))
        .map_err(|error| error.to_string())?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .await
        .map_err(|error| error.to_string())?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("plugin surface"),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits()),
            ..Default::default()
        })
        .await
        .map_err(|error| error.to_string())?;
    let mut configuration = surface
        .get_default_config(&adapter, 1, 1)
        .ok_or_else(|| "the plugin canvas is not supported by this adapter".to_owned())?;
    let format = preferred_format(&surface.get_capabilities(&adapter).formats)
        .unwrap_or(configuration.format);
    configuration.format = format;
    configuration.view_formats = vec![format];
    let gpu = Rc::new(Gpu {
        canvas,
        device,
        queue,
        surface,
        format,
        configuration: RefCell::new(configuration),
    });
    GPU.with(|current| *current.borrow_mut() = Some(gpu));
    Ok(())
}

pub(crate) struct Surface {
    gpu: Rc<Gpu>,
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
        let gpu = GPU
            .with(|gpu| gpu.borrow().clone())
            .ok_or_else(|| "the plugin canvas is not ready".to_owned())?;
        gpu.configure(&layout);
        Ok(Self {
            panes: Panes::new(gpu.format),
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
        self.gpu.configure(&layout);
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
        let frame = match self.gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.gpu.configure(&self.layout);
                return Ok(Vec::new());
            }
            status => return Err(format!("the plugin canvas is unavailable: {status:?}")),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
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
        frame.present();
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

fn preferred_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    formats
        .iter()
        .copied()
        .find(|format| {
            matches!(
                format,
                wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
            )
        })
        .or_else(|| formats.first().copied())
}
