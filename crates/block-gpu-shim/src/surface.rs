use block_gpu_abi as abi;

#[derive(Clone, Debug)]
struct Display;

impl wgpu::rwh::HasDisplayHandle for Display {
    fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
        Ok(wgpu::rwh::DisplayHandle::web())
    }
}

pub(crate) struct Canvas {
    canvas: web_sys::OffscreenCanvas,
    surface: wgpu::Surface<'static>,
    configuration: Option<wgpu::SurfaceConfiguration>,
    formats: Vec<wgpu::TextureFormat>,
    alpha: wgpu::CompositeAlphaMode,
    frame: Option<wgpu::SurfaceTexture>,
}

impl Canvas {
    pub(crate) async fn open(
        canvas: web_sys::OffscreenCanvas,
    ) -> Result<(Self, wgpu::Device, wgpu::Queue), String> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.display = Some(Box::new(Display));
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
                label: Some("plugin canvas"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|error| error.to_string())?;
        let capabilities = surface.get_capabilities(&adapter);
        let alpha = capabilities
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
        let canvas = Self {
            canvas,
            surface,
            configuration: None,
            formats: capabilities.formats,
            alpha,
            frame: None,
        };
        Ok((canvas, device, queue))
    }

    pub(crate) fn configure(
        &mut self,
        device: &wgpu::Device,
        requested: &abi::SurfaceConfiguration,
    ) -> Result<(), String> {
        let format = format(requested.format);
        if !self.formats.contains(&format) {
            return Err(format!(
                "this browser cannot show a plugin canvas as {format:?}"
            ));
        }
        let width = requested.width.max(1);
        let height = requested.height.max(1);
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        let configuration = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 1,
            alpha_mode: self.alpha,
            view_formats: vec![format],
        };
        self.surface.configure(device, &configuration);
        self.configuration = Some(configuration);
        Ok(())
    }

    pub(crate) fn acquire(&mut self, device: &wgpu::Device) -> Result<wgpu::Texture, String> {
        let Some(configuration) = self.configuration.clone() else {
            return Err("a plugin drew before its canvas was configured".to_owned());
        };
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(device, &configuration);
                match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                    status => return Err(format!("the plugin canvas is unavailable: {status:?}")),
                }
            }
            status => return Err(format!("the plugin canvas is unavailable: {status:?}")),
        };
        let texture = frame.texture.clone();
        self.frame = Some(frame);
        Ok(texture)
    }

    pub(crate) fn present(&mut self) {
        if let Some(frame) = self.frame.take() {
            frame.present();
        }
    }
}

fn format(format: abi::TextureFormat) -> wgpu::TextureFormat {
    block_gpu_host::texture_format(format)
}
