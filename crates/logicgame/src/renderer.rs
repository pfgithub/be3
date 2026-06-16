use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::{self, wgpu};
use logicgame::grid::{
    Component, ComponentKind, ComponentLead, ComponentSide, ConnectionDirection, ConnectionSlot,
    GridBounds, Orientation, Rotation, Wire,
};

const BACKGROUND_COLOR: [f32; 4] = [0.035, 0.043, 0.055, 1.0];
const MINOR_GRID_COLOR: [f32; 4] = [0.10, 0.12, 0.15, 1.0];
const MAJOR_GRID_COLOR: [f32; 4] = [0.18, 0.21, 0.26, 1.0];
const AXIS_COLOR: [f32; 4] = [0.35, 0.39, 0.48, 1.0];

#[derive(Clone)]
pub struct RenderFrame {
    pub viewport_size: [f32; 2],
    pub camera_center: [f32; 2],
    pub zoom: f32,
    pub grid_scale: f32,
    pub wires: Vec<DrawWire>,
    pub wire_values: Vec<WireValue>,
    pub triangles: Vec<DrawTriangle>,
}

#[derive(Clone, Copy, Debug)]
pub struct DrawWire {
    wire: Wire,
    color: [f32; 4],
    value_index: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct WireValue {
    low: u32,
    high: u32,
}

impl DrawWire {
    pub fn new(wire: Wire, color: [f32; 4], value_index: u32) -> Self {
        Self {
            wire,
            color,
            value_index,
        }
    }
}

impl WireValue {
    pub fn new(value: u64) -> Self {
        Self {
            low: value as u32,
            high: (value >> 32) as u32,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DrawTriangle {
    positions: [[f32; 2]; 3],
    color: [f32; 4],
}

impl DrawTriangle {
    pub const WIRE_COLOR: [f32; 4] = [0.18, 0.78, 0.91, 1.0];
    pub const GATE_COLOR: [f32; 4] = [0.91, 0.91, 0.95, 1.0];
    pub const COMPONENT_BACKGROUND_COLOR: [f32; 4] = [0.11, 0.14, 0.19, 1.0];
    pub const INPUT_COLOR: [f32; 4] = [0.28, 0.78, 0.48, 1.0];
    pub const OUTPUT_COLOR: [f32; 4] = [0.98, 0.55, 0.22, 1.0];
    pub const PREVIEW_COLOR: [f32; 4] = [0.98, 0.78, 0.24, 0.78];
    pub const ERROR_COLOR: [f32; 4] = [0.95, 0.22, 0.25, 1.0];
    pub const HIGHLIGHT_COLOR: [f32; 4] = [1.0, 0.78, 0.15, 1.0];
    pub const BOUNDS_COLOR: [f32; 4] = [0.72, 0.50, 0.96, 1.0];

    fn new(positions: [[f32; 2]; 3], color: [f32; 4]) -> Self {
        Self { positions, color }
    }

    pub fn wire(wire: Wire, color: [f32; 4]) -> Vec<Self> {
        let scale = wire.scale.get() as f32;
        let half_scale = scale * 0.5;
        let (start, end) = match wire.orientation() {
            Orientation::Horizontal => (
                [
                    wire.start.x as f32 + half_scale,
                    wire.start.y as f32 + half_scale,
                ],
                [
                    wire.end.x as f32 + half_scale,
                    wire.start.y as f32 + half_scale,
                ],
            ),
            Orientation::Vertical => (
                [
                    wire.start.x as f32 + half_scale,
                    wire.start.y as f32 + half_scale,
                ],
                [
                    wire.start.x as f32 + half_scale,
                    wire.end.y as f32 + half_scale,
                ],
            ),
        };
        let line_radius = scale * 0.08;
        let line = match wire.orientation() {
            Orientation::Horizontal => [
                start[0],
                start[1] - line_radius,
                end[0],
                end[1] + line_radius,
            ],
            Orientation::Vertical => [
                start[0] - line_radius,
                start[1],
                end[0] + line_radius,
                end[1],
            ],
        };

        let mut triangles = Vec::with_capacity(10);
        triangles.extend(rectangle(line, color));
        triangles.extend(diamond(start, scale * 0.38, color));
        triangles.extend(diamond(end, scale * 0.38, color));
        triangles
    }

    pub fn wire_endpoints(wire: Wire, color: [f32; 4]) -> Vec<Self> {
        let scale = wire.scale.get() as f32;
        let half_scale = scale * 0.5;
        let start = [
            wire.start.x as f32 + half_scale,
            wire.start.y as f32 + half_scale,
        ];
        let end = [
            wire.end.x as f32 + half_scale,
            wire.end.y as f32 + half_scale,
        ];

        let mut triangles = Vec::with_capacity(8);
        triangles.extend(diamond(start, scale * 0.38, color));
        triangles.extend(diamond(end, scale * 0.38, color));
        triangles
    }

    pub fn wire_endpoint_highlight(wire: Wire, endpoint: logicgame::grid::Point) -> Vec<Self> {
        let scale = wire.scale.get() as f32;
        diamond(
            [
                endpoint.x as f32 + scale * 0.5,
                endpoint.y as f32 + scale * 0.5,
            ],
            scale * 0.5,
            Self::HIGHLIGHT_COLOR,
        )
        .to_vec()
    }

    pub fn component(component: &Component, invalid: bool) -> Vec<Self> {
        let Some(size) = component.size() else {
            return Vec::new();
        };
        let min = [component.position.x as f32, component.position.y as f32];
        let extent = [size.width as f32, size.height as f32];
        let outline_color = if invalid {
            Self::ERROR_COLOR
        } else {
            Self::GATE_COLOR
        };
        let bounds = [min[0], min[1], min[0] + extent[0], min[1] + extent[1]];
        let thickness = (extent[0].min(extent[1]) * 0.08).clamp(0.06, 0.16);
        let mut triangles = Vec::new();
        triangles.extend(rectangle(bounds, Self::COMPONENT_BACKGROUND_COLOR));
        triangles.extend(outline(bounds, thickness, outline_color));
        for connection in component.connection_slots() {
            triangles.extend(connection_marker(
                component,
                connection,
                connection.scale.get() as f32 * 0.4,
            ));
        }

        match component.kind {
            ComponentKind::Not { .. } => {
                let canonical = [[0.12, 0.82], [0.88, 0.82], [0.5, 0.12]];
                let positions = canonical.map(|point| {
                    let point = rotate_point(point, component.rotation);
                    [min[0] + point[0] * extent[0], min[1] + point[1] * extent[1]]
                });
                triangles.push(Self::new(positions, outline_color));
            }
            ComponentKind::MergerSplitter {
                input_scale,
                output_scale,
            } if input_scale <= output_scale => {
                for canonical in [
                    [[0.12, 0.82], [0.88, 0.82], [0.62, 0.5]],
                    [[0.12, 0.18], [0.62, 0.5], [0.88, 0.18]],
                ] {
                    let positions = canonical.map(|point| {
                        let point = rotate_point(point, component.rotation);
                        [min[0] + point[0] * extent[0], min[1] + point[1] * extent[1]]
                    });
                    triangles.push(Self::new(positions, outline_color));
                }
            }
            ComponentKind::MergerSplitter { .. } => {
                for canonical in [
                    [[0.12, 0.82], [0.38, 0.5], [0.88, 0.82]],
                    [[0.12, 0.18], [0.88, 0.18], [0.38, 0.5]],
                ] {
                    let positions = canonical.map(|point| {
                        let point = rotate_point(point, component.rotation);
                        [min[0] + point[0] * extent[0], min[1] + point[1] * extent[1]]
                    });
                    triangles.push(Self::new(positions, outline_color));
                }
            }
            ComponentKind::Led => {
                let center = [min[0] + extent[0] * 0.5, min[1] + extent[1] * 0.5];
                triangles.extend(diamond(
                    center,
                    extent[0].min(extent[1]) * 0.32,
                    outline_color,
                ));
            }
            ComponentKind::Storage { .. } => triangles.extend(rectangle(
                [
                    min[0] + extent[0] * 0.22,
                    min[1] + extent[1] * 0.18,
                    min[0] + extent[0] * 0.78,
                    min[1] + extent[1] * 0.82,
                ],
                outline_color,
            )),
            ComponentKind::Input { .. } | ComponentKind::Output { .. } => {}
            ComponentKind::Subcomponent { .. } => {}
        }

        triangles
    }

    pub fn component_lead(component: &Component, viewport: [f32; 4]) -> Vec<Self> {
        let color = match component.kind {
            ComponentKind::Input { .. } => Self::INPUT_COLOR,
            ComponentKind::Output { .. } => Self::OUTPUT_COLOR,
            _ => return Vec::new(),
        };
        let Some(lead) = component.lead() else {
            return Vec::new();
        };
        let radius = (component.kind.snap().get() as f32 * 0.08).clamp(0.06, 0.16);
        let [viewport_left, viewport_top, viewport_right, viewport_bottom] = viewport;
        let center = (lead.start + lead.end) as f32 * 0.5;
        let rect = match lead {
            ComponentLead {
                side: ComponentSide::Top,
                axis,
                ..
            } => [
                center - radius,
                viewport_top,
                center + radius,
                (axis as f32).min(viewport_bottom),
            ],
            ComponentLead {
                side: ComponentSide::Right,
                axis,
                ..
            } => [
                (axis as f32).max(viewport_left),
                center - radius,
                viewport_right,
                center + radius,
            ],
            ComponentLead {
                side: ComponentSide::Bottom,
                axis,
                ..
            } => [
                center - radius,
                (axis as f32).max(viewport_top),
                center + radius,
                viewport_bottom,
            ],
            ComponentLead {
                side: ComponentSide::Left,
                axis,
                ..
            } => [
                viewport_left,
                center - radius,
                (axis as f32).min(viewport_right),
                center + radius,
            ],
        };
        if rect[0] < rect[2] && rect[1] < rect[3] {
            rectangle(rect, color).to_vec()
        } else {
            Vec::new()
        }
    }

    pub fn component_highlight(component: &Component) -> Vec<Self> {
        let Some(size) = component.size() else {
            return Vec::new();
        };
        let left = component.position.x as f32;
        let top = component.position.y as f32;
        let right = left + size.width as f32;
        let bottom = top + size.height as f32;
        let thickness = (size.width.min(size.height) as f32 * 0.08).clamp(0.08, 0.2);
        let color = Self::HIGHLIGHT_COLOR;

        let mut triangles = Vec::with_capacity(8);
        triangles.extend(rectangle([left, top, right, top + thickness], color));
        triangles.extend(rectangle([left, bottom - thickness, right, bottom], color));
        triangles.extend(rectangle(
            [left, top + thickness, left + thickness, bottom - thickness],
            color,
        ));
        triangles.extend(rectangle(
            [
                right - thickness,
                top + thickness,
                right,
                bottom - thickness,
            ],
            color,
        ));
        triangles
    }

    pub fn connection_highlight(component: &Component, connection: ConnectionSlot) -> Vec<Self> {
        let Some(size) = component.size() else {
            return Vec::new();
        };
        let left = component.position.x as f32;
        let top = component.position.y as f32;
        let right = left + size.width as f32;
        let bottom = top + size.height as f32;
        let start = connection.start as f32;
        let end = connection.end as f32;
        let thickness = (size.width.min(size.height) as f32 * 0.12).clamp(0.12, 0.3);
        let rect = match connection.side {
            ComponentSide::Top => [start, top - thickness * 0.5, end, top + thickness * 0.5],
            ComponentSide::Right => [right - thickness * 0.5, start, right + thickness * 0.5, end],
            ComponentSide::Bottom => [
                start,
                bottom - thickness * 0.5,
                end,
                bottom + thickness * 0.5,
            ],
            ComponentSide::Left => [left - thickness * 0.5, start, left + thickness * 0.5, end],
        };
        rectangle(rect, Self::HIGHLIGHT_COLOR).to_vec()
    }

    pub fn bounds(bounds: GridBounds, thickness: f32) -> Vec<Self> {
        let half_thickness = thickness * 0.5;
        outline(
            [
                bounds.min.x as f32 - half_thickness,
                bounds.min.y as f32 - half_thickness,
                bounds.max.x as f32 + half_thickness,
                bounds.max.y as f32 + half_thickness,
            ],
            thickness,
            Self::BOUNDS_COLOR,
        )
        .to_vec()
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }
}

fn rotate_point(point: [f32; 2], rotation: Rotation) -> [f32; 2] {
    match rotation {
        Rotation::Up => point,
        Rotation::Right => [1.0 - point[1], point[0]],
        Rotation::Down => [1.0 - point[0], 1.0 - point[1]],
        Rotation::Left => [point[1], 1.0 - point[0]],
    }
}

fn rectangle(rect: [f32; 4], color: [f32; 4]) -> [DrawTriangle; 2] {
    let [left, top, right, bottom] = rect;
    [
        DrawTriangle::new([[left, top], [right, top], [left, bottom]], color),
        DrawTriangle::new([[left, bottom], [right, top], [right, bottom]], color),
    ]
}

fn outline(rect: [f32; 4], thickness: f32, color: [f32; 4]) -> [DrawTriangle; 8] {
    let [left, top, right, bottom] = rect;
    let [top_a, top_b] = rectangle([left, top, right, top + thickness], color);
    let [bottom_a, bottom_b] = rectangle([left, bottom - thickness, right, bottom], color);
    let [left_a, left_b] = rectangle(
        [left, top + thickness, left + thickness, bottom - thickness],
        color,
    );
    let [right_a, right_b] = rectangle(
        [
            right - thickness,
            top + thickness,
            right,
            bottom - thickness,
        ],
        color,
    );
    [
        top_a, top_b, bottom_a, bottom_b, left_a, left_b, right_a, right_b,
    ]
}

fn connection_marker(
    component: &Component,
    connection: ConnectionSlot,
    radius: f32,
) -> [DrawTriangle; 1] {
    let center = match connection.side {
        ComponentSide::Top | ComponentSide::Bottom => [
            (connection.start + connection.end) as f32 * 0.5,
            match connection.side {
                ComponentSide::Top => component.position.y as f32,
                ComponentSide::Bottom => {
                    component.position.y as f32
                        + component.size().map_or(0.0, |size| size.height as f32)
                }
                _ => unreachable!(),
            },
        ],
        ComponentSide::Right | ComponentSide::Left => [
            match connection.side {
                ComponentSide::Right => {
                    component.position.x as f32
                        + component.size().map_or(0.0, |size| size.width as f32)
                }
                ComponentSide::Left => component.position.x as f32,
                _ => unreachable!(),
            },
            (connection.start + connection.end) as f32 * 0.5,
        ],
    };
    let color = match connection.direction {
        ConnectionDirection::Input => DrawTriangle::INPUT_COLOR,
        ConnectionDirection::Output => DrawTriangle::OUTPUT_COLOR,
    };
    let inward = match connection.side {
        ComponentSide::Top => [0.0, radius],
        ComponentSide::Right => [-radius, 0.0],
        ComponentSide::Bottom => [0.0, -radius],
        ComponentSide::Left => [radius, 0.0],
    };
    let tangent = match connection.side {
        ComponentSide::Top | ComponentSide::Bottom => [radius, 0.0],
        ComponentSide::Right | ComponentSide::Left => [0.0, radius],
    };
    let inside = [center[0] + inward[0], center[1] + inward[1]];
    let (tip, base_center) = match connection.direction {
        ConnectionDirection::Input => (inside, center),
        ConnectionDirection::Output => (center, inside),
    };
    let base_start = [base_center[0] - tangent[0], base_center[1] - tangent[1]];
    let base_end = [base_center[0] + tangent[0], base_center[1] + tangent[1]];

    [DrawTriangle::new([tip, base_start, base_end], color)]
}

fn diamond(center: [f32; 2], radius: f32, color: [f32; 4]) -> [DrawTriangle; 4] {
    let [x, y] = center;
    let top = [x, y - radius];
    let right = [x + radius, y];
    let bottom = [x, y + radius];
    let left = [x - radius, y];
    [
        DrawTriangle::new([center, top, right], color),
        DrawTriangle::new([center, right, bottom], color),
        DrawTriangle::new([center, bottom, left], color),
        DrawTriangle::new([center, left, top], color),
    ]
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RenderVertex {
    position: [f32; 2],
    fill_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WireVertex {
    position: [f32; 2],
    bit_coord: f32,
    scale: f32,
    value_index: u32,
    fill_color: [f32; 4],
}

impl RenderVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

impl WireVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
            0 => Float32x2,
            2 => Float32,
            3 => Float32,
            4 => Uint32,
            5 => Float32x4
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

pub struct GridCallback {
    pub frame: RenderFrame,
}

impl egui_wgpu::CallbackTrait for GridCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let renderer = callback_resources
            .get_mut::<GridRenderer>()
            .expect("grid renderer was not initialized");
        renderer.prepare(device, queue, &self.frame);
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let renderer = callback_resources
            .get::<GridRenderer>()
            .expect("grid renderer was not initialized");
        renderer.paint(render_pass);
    }
}

pub struct GridRenderer {
    triangle_pipeline: wgpu::RenderPipeline,
    wire_pipeline: wgpu::RenderPipeline,
    wire_value_bind_group_layout: wgpu::BindGroupLayout,
    wire_value_bind_group: wgpu::BindGroup,
    background_vertex_buffer: Arc<wgpu::Buffer>,
    vertex_buffer: Arc<wgpu::Buffer>,
    wire_vertex_buffer: Arc<wgpu::Buffer>,
    wire_value_buffer: Arc<wgpu::Buffer>,
    background_vertex_capacity: usize,
    vertex_capacity: usize,
    wire_vertex_capacity: usize,
    wire_value_capacity: usize,
    background_vertex_count: u32,
    vertex_count: u32,
    wire_vertex_count: u32,
}

impl GridRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("editor.wgsl"));
        let triangle_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("logic triangle pipeline layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });
        let wire_value_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("logic wire value bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let wire_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("logic wire pipeline layout"),
            bind_group_layouts: &[Some(&wire_value_bind_group_layout)],
            immediate_size: 0,
        });
        let triangle_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("logic triangle pipeline"),
            layout: Some(&triangle_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[RenderVertex::layout()],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let wire_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("logic wire pipeline"),
            layout: Some(&wire_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("wire_vertex"),
                compilation_options: Default::default(),
                buffers: &[WireVertex::layout()],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("wire_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let vertex_capacity = 1;
        let background_vertex_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("logic background vertex buffer"),
            size: std::mem::size_of::<RenderVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        let vertex_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("logic triangle vertex buffer"),
            size: std::mem::size_of::<RenderVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        let wire_vertex_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("logic wire vertex buffer"),
            size: std::mem::size_of::<WireVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        let wire_value_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("logic wire value buffer"),
            size: std::mem::size_of::<WireValue>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        let wire_value_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("logic wire value bind group"),
            layout: &wire_value_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wire_value_buffer.as_entire_binding(),
            }],
        });

        Self {
            triangle_pipeline,
            wire_pipeline,
            wire_value_bind_group_layout,
            wire_value_bind_group,
            background_vertex_buffer,
            vertex_buffer,
            wire_vertex_buffer,
            wire_value_buffer,
            background_vertex_capacity: vertex_capacity,
            vertex_capacity,
            wire_vertex_capacity: vertex_capacity,
            wire_value_capacity: vertex_capacity,
            background_vertex_count: 0,
            vertex_count: 0,
            wire_vertex_count: 0,
        }
    }

    fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &RenderFrame) {
        let background_vertices = triangle_vertices(background_triangles(frame), frame);
        prepare_vertex_buffer(
            device,
            queue,
            "logic background vertex buffer",
            &background_vertices,
            &mut self.background_vertex_buffer,
            &mut self.background_vertex_capacity,
        );
        self.background_vertex_count = background_vertices.len() as u32;

        let vertices = triangle_vertices(frame.triangles.clone(), frame);
        prepare_vertex_buffer(
            device,
            queue,
            "logic triangle vertex buffer",
            &vertices,
            &mut self.vertex_buffer,
            &mut self.vertex_capacity,
        );
        self.vertex_count = vertices.len() as u32;

        let wire_vertices = wire_vertices(&frame.wires, frame);
        prepare_vertex_buffer(
            device,
            queue,
            "logic wire vertex buffer",
            &wire_vertices,
            &mut self.wire_vertex_buffer,
            &mut self.wire_vertex_capacity,
        );
        self.wire_vertex_count = wire_vertices.len() as u32;

        let wire_values = if frame.wire_values.is_empty() {
            vec![WireValue::new(0)]
        } else {
            frame.wire_values.clone()
        };
        if wire_values.len() > self.wire_value_capacity {
            self.wire_value_capacity = wire_values.len().next_power_of_two();
            self.wire_value_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("logic wire value buffer"),
                size: (self.wire_value_capacity * std::mem::size_of::<WireValue>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.wire_value_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("logic wire value bind group"),
                layout: &self.wire_value_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.wire_value_buffer.as_entire_binding(),
                }],
            });
        }
        queue.write_buffer(
            &self.wire_value_buffer,
            0,
            bytemuck::cast_slice(&wire_values),
        );
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        if self.background_vertex_count > 0 {
            render_pass.set_pipeline(&self.triangle_pipeline);
            render_pass.set_vertex_buffer(0, self.background_vertex_buffer.slice(..));
            render_pass.draw(0..self.background_vertex_count, 0..1);
        }
        if self.wire_vertex_count > 0 {
            render_pass.set_pipeline(&self.wire_pipeline);
            render_pass.set_bind_group(0, &self.wire_value_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.wire_vertex_buffer.slice(..));
            render_pass.draw(0..self.wire_vertex_count, 0..1);
        }
        if self.vertex_count > 0 {
            render_pass.set_pipeline(&self.triangle_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..self.vertex_count, 0..1);
        }
    }
}

fn prepare_vertex_buffer<T: Pod>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &'static str,
    vertices: &[T],
    buffer: &mut Arc<wgpu::Buffer>,
    capacity: &mut usize,
) {
    if vertices.len() > *capacity {
        *capacity = vertices.len().next_power_of_two();
        *buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: (*capacity * std::mem::size_of::<T>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }
    if !vertices.is_empty() {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(vertices));
    }
}

fn world_to_clip(position: [f32; 2], frame: &RenderFrame) -> [f32; 2] {
    [
        (position[0] - frame.camera_center[0]) * frame.zoom / frame.viewport_size[0] * 2.0,
        1.0 - ((position[1] - frame.camera_center[1]) * frame.zoom + frame.viewport_size[1] * 0.5)
            / frame.viewport_size[1]
            * 2.0,
    ]
}

#[cfg(test)]
fn frame_triangles(frame: &RenderFrame) -> Vec<DrawTriangle> {
    let mut triangles = background_triangles(frame);
    triangles.extend_from_slice(&frame.triangles);
    triangles
}

fn background_triangles(frame: &RenderFrame) -> Vec<DrawTriangle> {
    let half_width = frame.viewport_size[0] / frame.zoom * 0.5;
    let half_height = frame.viewport_size[1] / frame.zoom * 0.5;
    let bounds = [
        frame.camera_center[0] - half_width,
        frame.camera_center[1] - half_height,
        frame.camera_center[0] + half_width,
        frame.camera_center[1] + half_height,
    ];
    let mut triangles = Vec::new();
    triangles.extend(rectangle(bounds, BACKGROUND_COLOR));
    add_grid_lines(
        &mut triangles,
        bounds,
        frame.grid_scale,
        1.5 / frame.zoom,
        MINOR_GRID_COLOR,
    );
    add_grid_lines(
        &mut triangles,
        bounds,
        frame.grid_scale * 8.0,
        2.0 / frame.zoom,
        MAJOR_GRID_COLOR,
    );
    add_axis_lines(&mut triangles, bounds, 3.0 / frame.zoom);
    triangles
}

fn triangle_vertices(triangles: Vec<DrawTriangle>, frame: &RenderFrame) -> Vec<RenderVertex> {
    let mut vertices = Vec::with_capacity(triangles.len() * 3);
    for triangle in triangles {
        vertices.extend(triangle.positions.map(|position| RenderVertex {
            position: world_to_clip(position, frame),
            fill_color: triangle.color,
        }));
    }
    vertices
}

fn wire_vertices(wires: &[DrawWire], frame: &RenderFrame) -> Vec<WireVertex> {
    let mut vertices = Vec::with_capacity(wires.len() * 6);
    for draw in wires {
        let scale = draw.wire.scale.get() as f32;
        let half_scale = scale * 0.5;
        let half_thickness = (scale * 0.36).max(0.08);
        let (left, top, right, bottom) = match draw.wire.orientation() {
            Orientation::Horizontal => {
                let start_x = draw.wire.start.x as f32 + half_scale;
                let end_x = draw.wire.end.x as f32 + half_scale;
                let center_y = draw.wire.start.y as f32 + half_scale;
                (
                    start_x,
                    center_y - half_thickness,
                    end_x,
                    center_y + half_thickness,
                )
            }
            Orientation::Vertical => {
                let center_x = draw.wire.start.x as f32 + half_scale;
                let start_y = draw.wire.start.y as f32 + half_scale;
                let end_y = draw.wire.end.y as f32 + half_scale;
                (
                    center_x - half_thickness,
                    start_y,
                    center_x + half_thickness,
                    end_y,
                )
            }
        };
        let horizontal = draw.wire.orientation() == Orientation::Horizontal;
        let top_left = wire_vertex([left, top], 0.0, scale, draw, frame);
        let top_right = wire_vertex(
            [right, top],
            if horizontal { 0.0 } else { scale },
            scale,
            draw,
            frame,
        );
        let bottom_left = wire_vertex(
            [left, bottom],
            if horizontal { scale } else { 0.0 },
            scale,
            draw,
            frame,
        );
        let bottom_right = wire_vertex([right, bottom], scale, scale, draw, frame);
        vertices.extend([
            top_left,
            top_right,
            bottom_left,
            bottom_left,
            top_right,
            bottom_right,
        ]);
    }
    vertices
}

fn wire_vertex(
    position: [f32; 2],
    bit_coord: f32,
    scale: f32,
    draw: &DrawWire,
    frame: &RenderFrame,
) -> WireVertex {
    WireVertex {
        position: world_to_clip(position, frame),
        bit_coord,
        scale,
        value_index: draw.value_index,
        fill_color: draw.color,
    }
}

fn add_grid_lines(
    triangles: &mut Vec<DrawTriangle>,
    bounds: [f32; 4],
    spacing: f32,
    width: f32,
    color: [f32; 4],
) {
    let [left, top, right, bottom] = bounds;
    let mut x = (left / spacing).floor() * spacing;
    while x <= right {
        let line_left = (x - width * 0.5).max(left);
        let line_right = (x + width * 0.5).min(right);
        if line_left < line_right {
            triangles.extend(rectangle([line_left, top, line_right, bottom], color));
        }
        x += spacing;
    }
    let mut y = (top / spacing).floor() * spacing;
    while y <= bottom {
        let line_top = (y - width * 0.5).max(top);
        let line_bottom = (y + width * 0.5).min(bottom);
        if line_top < line_bottom {
            triangles.extend(rectangle([left, line_top, right, line_bottom], color));
        }
        y += spacing;
    }
}

fn add_axis_lines(triangles: &mut Vec<DrawTriangle>, bounds: [f32; 4], width: f32) {
    let [left, top, right, bottom] = bounds;
    if left <= 0.0 && 0.0 <= right {
        let line_left = (-width * 0.5).max(left);
        let line_right = (width * 0.5).min(right);
        triangles.extend(rectangle([line_left, top, line_right, bottom], AXIS_COLOR));
    }
    if top <= 0.0 && 0.0 <= bottom {
        let line_top = (-width * 0.5).max(top);
        let line_bottom = (width * 0.5).min(bottom);
        triangles.extend(rectangle([left, line_top, right, line_bottom], AXIS_COLOR));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use logicgame::grid::{
        ComponentId, ComponentSide, ConnectionDirection, ConnectionSlot, ConnectionSlotId, InputId,
        OutputId, Point, Scale,
    };

    #[test]
    fn wires_and_components_are_emitted_as_filled_triangles() {
        let wire = Wire::new(Point::new(0, 0), Point::new(4, 0), Scale::ONE).unwrap();
        let wire_triangles = DrawTriangle::wire(wire, DrawTriangle::WIRE_COLOR);
        assert_eq!(wire_triangles.len(), 10);
        assert!(wire_triangles
            .iter()
            .all(|triangle| triangle.color == DrawTriangle::WIRE_COLOR));
        assert_eq!(wire_triangles[2].positions[0], [0.5, 0.5]);
        assert_eq!(wire_triangles[6].positions[0], [4.5, 0.5]);
        let endpoint_triangles = DrawTriangle::wire_endpoints(wire, DrawTriangle::WIRE_COLOR);
        assert_eq!(endpoint_triangles.len(), 8);
        assert_eq!(endpoint_triangles[0].positions[0], [0.5, 0.5]);
        assert_eq!(endpoint_triangles[4].positions[0], [4.5, 0.5]);

        let gate = Component {
            id: ComponentId(0),
            position: Point::new(10, 20),
            rotation: Rotation::Right,
            kind: ComponentKind::Not { scale: Scale::ONE },
        };
        let gate_triangles = DrawTriangle::component(&gate, false);
        assert_eq!(gate_triangles.len(), 13);
        assert!(gate_triangles[..2]
            .iter()
            .all(|triangle| triangle.color == DrawTriangle::COMPONENT_BACKGROUND_COLOR));
        assert!(gate_triangles
            .iter()
            .any(|triangle| triangle.color == DrawTriangle::INPUT_COLOR));
        assert!(gate_triangles
            .iter()
            .any(|triangle| triangle.color == DrawTriangle::OUTPUT_COLOR));
        let gate_triangle = gate_triangles[12];
        let tip = gate_triangle.positions[2];
        assert!(tip[0] > gate_triangle.positions[0][0]);
        assert!(tip[0] > gate_triangle.positions[1][0]);
        assert_eq!(gate_triangle.color, DrawTriangle::GATE_COLOR);

        let led = Component {
            id: ComponentId(1),
            position: Point::new(0, 0),
            rotation: Rotation::Up,
            kind: ComponentKind::Led,
        };
        let led_triangles = DrawTriangle::component(&led, false);
        assert_eq!(led_triangles.len(), 15);
        assert!(led_triangles
            .iter()
            .any(|triangle| triangle.color == DrawTriangle::INPUT_COLOR));

        let storage = Component {
            id: ComponentId(2),
            position: Point::new(0, 0),
            rotation: Rotation::Right,
            kind: ComponentKind::Storage {
                scale: Scale::ONE,
                value: 0,
            },
        };
        let storage_triangles = DrawTriangle::component(&storage, false);
        assert_eq!(storage_triangles.len(), 14);
        assert!(storage_triangles
            .iter()
            .any(|triangle| triangle.color == DrawTriangle::INPUT_COLOR));
        assert!(storage_triangles
            .iter()
            .any(|triangle| triangle.color == DrawTriangle::OUTPUT_COLOR));

        let invalid_gate = DrawTriangle::component(&gate, true);
        assert!(invalid_gate[2..10]
            .iter()
            .all(|triangle| triangle.color == DrawTriangle::ERROR_COLOR));

        let highlight = DrawTriangle::component_highlight(&gate);
        assert_eq!(highlight.len(), 8);
        assert!(highlight
            .iter()
            .all(|triangle| triangle.color == DrawTriangle::HIGHLIGHT_COLOR));

        let connection = DrawTriangle::connection_highlight(
            &gate,
            ConnectionSlot {
                id: ConnectionSlotId(0),
                direction: ConnectionDirection::Input,
                side: ComponentSide::Bottom,
                start: 10,
                end: 12,
                scale: Scale::ONE,
            },
        );
        assert_eq!(connection.len(), 2);
        assert!(connection
            .iter()
            .all(|triangle| triangle.color == DrawTriangle::HIGHLIGHT_COLOR));
    }

    #[test]
    fn connection_markers_are_inward_and_stay_inside_component_bounds() {
        let component = Component {
            id: ComponentId(0),
            position: Point::new(10, 20),
            rotation: Rotation::Up,
            kind: ComponentKind::Storage {
                scale: Scale::ONE,
                value: 0,
            },
        };
        let radius = 0.2;

        for (side, start, end) in [
            (ComponentSide::Top, 10, 11),
            (ComponentSide::Right, 20, 22),
            (ComponentSide::Bottom, 10, 11),
            (ComponentSide::Left, 20, 22),
        ] {
            let inward = match side {
                ComponentSide::Top => [0.0, radius],
                ComponentSide::Right => [-radius, 0.0],
                ComponentSide::Bottom => [0.0, -radius],
                ComponentSide::Left => [radius, 0.0],
            };
            let boundary_center = match side {
                ComponentSide::Top => [10.5, 20.0],
                ComponentSide::Right => [11.0, 21.0],
                ComponentSide::Bottom => [10.5, 22.0],
                ComponentSide::Left => [10.0, 21.0],
            };
            let inside_center = [
                boundary_center[0] + inward[0],
                boundary_center[1] + inward[1],
            ];

            for direction in [ConnectionDirection::Input, ConnectionDirection::Output] {
                let marker = connection_marker(
                    &component,
                    ConnectionSlot {
                        id: ConnectionSlotId(0),
                        direction,
                        side,
                        start,
                        end,
                        scale: Scale::ONE,
                    },
                    radius,
                )[0];

                let expected_tip = match direction {
                    ConnectionDirection::Input => inside_center,
                    ConnectionDirection::Output => boundary_center,
                };
                assert_eq!(marker.positions[0], expected_tip);
                assert!(marker
                    .positions
                    .iter()
                    .all(|[x, y]| { (10.0..=11.0).contains(x) && (20.0..=22.0).contains(y) }));
            }
        }
    }

    #[test]
    fn input_and_output_leads_extend_to_the_viewport_edge() {
        let viewport = [-5.0, -5.0, 5.0, 5.0];
        let input = Component {
            id: ComponentId(0),
            position: Point::new(0, 0),
            rotation: Rotation::Up,
            kind: ComponentKind::Input {
                scale: Scale::ONE,
                id: InputId(0),
            },
        };
        let output = Component {
            id: ComponentId(1),
            position: Point::new(-10, 0),
            rotation: Rotation::Right,
            kind: ComponentKind::Output {
                scale: Scale::ONE,
                id: OutputId(0),
            },
        };

        let input_lead = DrawTriangle::component_lead(&input, viewport);
        assert_eq!(input_lead.len(), 2);
        assert!(input_lead
            .iter()
            .all(|triangle| triangle.color == DrawTriangle::INPUT_COLOR));
        assert!(input_lead
            .iter()
            .flat_map(|triangle| triangle.positions)
            .any(|position| position[1] == viewport[1]));

        let output_lead = DrawTriangle::component_lead(&output, viewport);
        assert_eq!(output_lead.len(), 2);
        assert!(output_lead
            .iter()
            .all(|triangle| triangle.color == DrawTriangle::OUTPUT_COLOR));
        assert!(output_lead
            .iter()
            .flat_map(|triangle| triangle.positions)
            .any(|position| position[0] == viewport[2]));
    }

    #[test]
    fn wire_vertices_carry_segment_coordinates_and_value_indices() {
        let wire = Wire::new(Point::new(0, 0), Point::new(8, 0), Scale::new(4).unwrap()).unwrap();
        let frame = RenderFrame {
            viewport_size: [80.0, 80.0],
            camera_center: [0.0, 0.0],
            zoom: 10.0,
            grid_scale: 1.0,
            wires: Vec::new(),
            wire_values: vec![WireValue::new(0b1010)],
            triangles: Vec::new(),
        };
        let vertices = wire_vertices(&[DrawWire::new(wire, DrawTriangle::WIRE_COLOR, 7)], &frame);

        assert_eq!(vertices.len(), 6);
        assert_eq!(vertices[0].bit_coord, 0.0);
        assert_eq!(vertices[2].bit_coord, 4.0);
        assert!(vertices
            .iter()
            .all(|vertex| vertex.value_index == 7 && vertex.scale == 4.0));
        assert_eq!(frame.wire_values[0].low, 0b1010);
        assert_eq!(frame.wire_values[0].high, 0);
    }

    #[test]
    fn one_x_grid_emits_lines_one_world_unit_apart() {
        let frame = RenderFrame {
            viewport_size: [80.0, 80.0],
            camera_center: [0.0, 0.0],
            zoom: 10.0,
            grid_scale: 1.0,
            wires: Vec::new(),
            wire_values: Vec::new(),
            triangles: Vec::new(),
        };
        let triangles = frame_triangles(&frame);

        let first_minor_x = triangles[4].positions[0][0];
        let second_minor_x = triangles[6].positions[0][0];
        assert!((second_minor_x - first_minor_x - 1.0).abs() < 0.0001);
        assert_eq!(triangles[4].color, MINOR_GRID_COLOR);
        assert_eq!(triangles[6].color, MINOR_GRID_COLOR);
    }

    #[test]
    fn viewport_grid_and_entities_are_layered_and_bounded() {
        let entity = DrawTriangle::new(
            [[1.0, 1.0], [2.0, 1.0], [1.0, 2.0]],
            DrawTriangle::WIRE_COLOR,
        );
        let frame = RenderFrame {
            viewport_size: [100.0, 100.0],
            camera_center: [0.0, 0.0],
            zoom: 10.0,
            grid_scale: 1.0,
            wires: Vec::new(),
            wire_values: Vec::new(),
            triangles: vec![entity],
        };

        let triangles = frame_triangles(&frame);
        assert_eq!(triangles[0].color, BACKGROUND_COLOR);
        assert_eq!(triangles.last().unwrap().color, DrawTriangle::WIRE_COLOR);

        for triangle in triangles.iter().filter(|triangle| {
            triangle.color == MINOR_GRID_COLOR
                || triangle.color == MAJOR_GRID_COLOR
                || triangle.color == AXIS_COLOR
        }) {
            assert!(triangle
                .positions
                .iter()
                .all(|[x, y]| { -5.0 <= *x && *x <= 5.0 && -5.0 <= *y && *y <= 5.0 }));
        }
    }
}
