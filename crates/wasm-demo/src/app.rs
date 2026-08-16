use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

struct Scene {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    time_buffer: wgpu::Buffer,
    time_bind_group: wgpu::BindGroup,
    configured_size: [u32; 2],
    start_time_ms: f64,
}

/// Loads and runs the wasm-demo scene into `canvas_id`, using its own wgpu
/// device rather than block-app's: wgpu has no supported way to hand a live
/// device across a wasm instance boundary, so the guest sets one up itself,
/// exactly like block-app's own web entry point does. The canvas is never
/// shown; block-app copies its contents into its own texture every frame and
/// drives its resolution by setting `width`/`height` on it directly, so the
/// render loop tracks those attributes rather than CSS layout size.
#[wasm_bindgen]
pub async fn start(canvas_id: String) -> Result<(), JsValue> {
    let window =
        web_sys::window().ok_or_else(|| JsValue::from_str("no browser window is available"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document is available"))?;
    let canvas = document
        .get_element_by_id(&canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("no element id {canvas_id}")))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let instance = wgpu::Instance::default();
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .map_err(|error| JsValue::from_str(&error.to_string()))?;

    let shader = device.create_shader_module(wgpu::include_wgsl!("scene.wgsl"));

    let time_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("wasm demo time bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("wasm demo pipeline layout"),
        bind_group_layouts: &[Some(&time_bind_group_layout)],
        immediate_size: 0,
    });

    let format = surface
        .get_capabilities(&adapter)
        .formats
        .first()
        .copied()
        .ok_or_else(|| JsValue::from_str("the surface has no supported texture formats"))?;
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("wasm demo pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vertex_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fragment_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });

    let time_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("wasm demo time buffer"),
        size: std::mem::size_of::<f32>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let time_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("wasm demo time bind group"),
        layout: &time_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: time_buffer.as_entire_binding(),
        }],
    });

    let scene = Rc::new(RefCell::new(Scene {
        surface,
        device,
        queue,
        pipeline,
        time_buffer,
        time_bind_group,
        configured_size: [0, 0],
        start_time_ms: window
            .performance()
            .map_or(0.0, |performance| performance.now()),
    }));

    start_render_loop(canvas, adapter, scene);
    Ok(())
}

type FrameClosure = Closure<dyn FnMut(f64)>;

fn start_render_loop(
    canvas: web_sys::HtmlCanvasElement,
    adapter: wgpu::Adapter,
    scene: Rc<RefCell<Scene>>,
) {
    let closure_slot: Rc<RefCell<Option<FrameClosure>>> = Rc::new(RefCell::new(None));
    let closure_slot_for_body = closure_slot.clone();

    *closure_slot.borrow_mut() = Some(Closure::new(move |time_ms: f64| {
        render_frame(&canvas, &adapter, &scene, time_ms);
        request_animation_frame(
            closure_slot_for_body
                .borrow()
                .as_ref()
                .expect("set just below"),
        );
    }));
    request_animation_frame(closure_slot.borrow().as_ref().expect("set just above"));
}

fn request_animation_frame(closure: &FrameClosure) {
    web_sys::window()
        .expect("no global window exists")
        .request_animation_frame(closure.as_ref().unchecked_ref())
        .expect("requestAnimationFrame is supported");
}

fn render_frame(
    canvas: &web_sys::HtmlCanvasElement,
    adapter: &wgpu::Adapter,
    scene: &Rc<RefCell<Scene>>,
    time_ms: f64,
) {
    let mut scene = scene.borrow_mut();

    let width = canvas.width();
    let height = canvas.height();
    if width == 0 || height == 0 {
        return;
    }
    if scene.configured_size != [width, height] {
        let Some(config) = scene.surface.get_default_config(adapter, width, height) else {
            return;
        };
        scene.surface.configure(&scene.device, &config);
        scene.configured_size = [width, height];
    }

    let output = match scene.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(texture)
        | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
        _ => return,
    };
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let time_seconds = ((time_ms - scene.start_time_ms) / 1000.0) as f32;
    scene
        .queue
        .write_buffer(&scene.time_buffer, 0, &time_seconds.to_le_bytes());

    let mut encoder = scene
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("wasm demo encoder"),
        });
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wasm demo pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.06,
                        g: 0.06,
                        b: 0.09,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        render_pass.set_pipeline(&scene.pipeline);
        render_pass.set_bind_group(0, &scene.time_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
    scene.queue.submit([encoder.finish()]);
    output.present();
}
