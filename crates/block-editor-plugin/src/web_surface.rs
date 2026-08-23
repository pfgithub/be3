use block_plugin_api::ScreenLayout;
use eframe::egui_wgpu::wgpu;
use std::time::Duration;

use crate::{panes::Panes, screens::Screens};

#[derive(Clone, Debug)]
struct WebDisplay;

impl wgpu::rwh::HasDisplayHandle for WebDisplay {
    fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
        Ok(wgpu::rwh::DisplayHandle::web())
    }
}

pub(crate) struct Surface {
    canvas: web_sys::HtmlCanvasElement,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    configuration: wgpu::SurfaceConfiguration,
    panes: Panes,
    configured: bool,
}

impl Surface {
    pub(crate) async fn new(canvas: web_sys::HtmlCanvasElement) -> Result<Self, String> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL;
        descriptor.display = Some(Box::new(WebDisplay));
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
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
        Ok(Self {
            canvas,
            device,
            queue,
            surface,
            configuration,
            panes: Panes::new(format),
            configured: false,
        })
    }

    pub(crate) fn resize(&mut self, layout: &ScreenLayout) {
        let width = layout.width.max(1);
        let height = layout.height.max(1);
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        self.configuration.width = width;
        self.configuration.height = height;
        self.surface.configure(&self.device, &self.configuration);
        self.configured = true;
    }

    pub(crate) fn render(
        &mut self,
        screens: &mut Screens,
        layout: &ScreenLayout,
        time: f64,
    ) -> Result<Option<Duration>, String> {
        if !self.configured || layout.is_empty() {
            return Ok(None);
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.configuration);
                return Ok(None);
            }
            status => return Err(format!("the plugin canvas is unavailable: {status:?}")),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let painted = self.panes.paint(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            layout,
            screens,
            time,
        );
        self.queue
            .submit(painted.commands.into_iter().chain([encoder.finish()]));
        frame.present();
        Ok(painted.repaint)
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
