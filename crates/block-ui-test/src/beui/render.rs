use std::collections::BTreeMap;

use beui::{clear_color, Color32, FrameOutput, Renderer, Vec2};
use paint_snapshot::{Content, Frame, Primitive, Snapshot, Texture, Triangle, Vertex};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const ALIGNMENT: u32 = 256;
const SCREEN: paint_snapshot::TextureKey = 0;

pub(crate) fn capture(
    output: &FrameOutput,
    size: Vec2,
    pixels_per_point: f32,
    background: Color32,
) -> Result<Snapshot, String> {
    let width = (size.x * pixels_per_point).round().max(1.0) as u32;
    let height = (size.y * pixels_per_point).round().max(1.0) as u32;
    let pixels = render(output, [width, height], pixels_per_point, background)?;
    let texture = Texture::encode([width, height], &pixels)?;
    Ok(Snapshot::of(
        Frame {
            size: [width, height],
            pixels_per_point,
            background: background.to_array(),
            primitives: vec![Primitive {
                clip: [0.0, 0.0, size.x, size.y],
                content: Content::Mesh(quad(size)),
            }],
        },
        BTreeMap::from([(SCREEN, texture)]),
    ))
}

fn quad(size: Vec2) -> Vec<Triangle> {
    let corner = |x: f32, y: f32, u: f32, v: f32| Vertex {
        pos: [x, y],
        uv: [u, v],
        color: [255, 255, 255, 255],
    };
    let top_left = corner(0.0, 0.0, 0.0, 0.0);
    let top_right = corner(size.x, 0.0, 1.0, 0.0);
    let bottom_left = corner(0.0, size.y, 0.0, 1.0);
    let bottom_right = corner(size.x, size.y, 1.0, 1.0);
    vec![
        Triangle {
            texture: SCREEN,
            corners: [top_left, top_right, bottom_left],
        },
        Triangle {
            texture: SCREEN,
            corners: [top_right, bottom_right, bottom_left],
        },
    ]
}

fn render(
    output: &FrameOutput,
    size: [u32; 2],
    pixels_per_point: f32,
    background: Color32,
) -> Result<Vec<[u8; 4]>, String> {
    let [width, height] = size;
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .map_err(|error| format!("no graphics adapter is available: {error}"))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("beui editor device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|error| format!("the adapter did not provide a device: {error}"))?;

    let mut renderer = Renderer::new(&device, FORMAT);
    renderer.prepare(
        &device,
        &queue,
        output,
        beui::vec2(width as f32, height as f32),
        pixels_per_point,
    );

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("beui editor target"),
        size: wgpu::Extent3d {
            width,
            height,
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
    let stride = width * 4;
    let padded = stride.div_ceil(ALIGNMENT) * ALIGNMENT;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("beui editor readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("beui editor encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("beui editor pass"),
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
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| format!("the device never finished the frame: {error}"))?;
    let mapped = buffer.slice(..).get_mapped_range().to_vec();
    Ok(rows(&mapped, width, height, padded))
}

fn rows(mapped: &[u8], width: u32, height: u32, padded: u32) -> Vec<[u8; 4]> {
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for row in 0..height {
        let start = (row * padded) as usize;
        for column in 0..width as usize {
            let texel = start + column * 4;
            pixels.push([
                mapped[texel],
                mapped[texel + 1],
                mapped[texel + 2],
                mapped[texel + 3],
            ]);
        }
    }
    pixels
}
