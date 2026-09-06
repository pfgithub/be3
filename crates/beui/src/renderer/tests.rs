use super::*;

mod a_clip_rectangle_hides_what_falls_outside_it;
mod a_fill_with_fractional_bounds_lands_on_whole_pixels;
mod a_filled_rectangle_covers_its_bounds;
mod text_at_a_fractional_origin_lands_on_whole_pixels;
mod text_paints_glyphs_over_the_background;

use crate::context::Context;
use crate::font::FontId;
use crate::geometry::{pos2, vec2, Pos2};
use crate::input::RawInput;
use crate::painter::Painter;

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub(crate) struct Capture {
    pixels: Vec<u8>,
}

impl Capture {
    pub(crate) fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let offset = ((y * SIZE + x) * 4) as usize;
        let mut pixel = [0; 4];
        pixel.copy_from_slice(&self.pixels[offset..offset + 4]);
        pixel
    }

    pub(crate) fn brightest(&self, rect: Rect) -> u8 {
        let mut brightest = 0;
        for y in rect.top() as u32..rect.bottom() as u32 {
            for x in rect.left() as u32..rect.right() as u32 {
                brightest = brightest.max(self.pixel(x, y)[0]);
            }
        }
        brightest
    }
}

pub(crate) fn capture(background: Color32, paint: impl FnOnce(&Painter)) -> Capture {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .expect("no graphics adapter is available");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("beui test device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .expect("the adapter did not provide a device");

    let context = Context::new();
    let output = context.run(RawInput::default(), |context| paint(&context.painter()));

    let mut renderer = Renderer::new(&device, FORMAT);
    renderer.prepare(
        &device,
        &queue,
        &output,
        vec2(SIZE as f32, SIZE as f32),
        1.0,
    );

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("beui test target"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("beui test readback"),
        size: (SIZE * SIZE * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("beui test encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("beui test pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color(background)),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer.paint(&mut pass);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE * 4),
                rows_per_image: Some(SIZE),
            },
        },
        wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the device never finished the frame");
    let pixels = buffer.slice(..).get_mapped_range().to_vec();
    Capture { pixels }
}
