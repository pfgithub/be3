use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::{self, wgpu};
use logicgame::grid::{Component, ComponentKind, Orientation, Rotation, Wire};

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
    pub triangles: Vec<DrawTriangle>,
}

#[derive(Clone, Copy, Debug)]
pub struct DrawTriangle {
    positions: [[f32; 2]; 3],
    color: [f32; 4],
}

impl DrawTriangle {
    pub const WIRE_COLOR: [f32; 4] = [0.18, 0.78, 0.91, 1.0];
    pub const GATE_COLOR: [f32; 4] = [0.91, 0.91, 0.95, 1.0];
    pub const PREVIEW_COLOR: [f32; 4] = [0.98, 0.78, 0.24, 0.78];
    pub const ERROR_COLOR: [f32; 4] = [0.95, 0.22, 0.25, 1.0];
    pub const HIGHLIGHT_COLOR: [f32; 4] = [1.0, 0.78, 0.15, 1.0];

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

    pub fn component(component: &Component, invalid: bool) -> Option<Self> {
        let ComponentKind::Not { .. } = component.kind else {
            return None;
        };
        let size = component.size()?;
        let min = [component.position.x as f32, component.position.y as f32];
        let extent = [size.width as f32, size.height as f32];
        let canonical = [[0.12, 0.82], [0.88, 0.82], [0.5, 0.12]];
        let positions = canonical.map(|point| {
            let point = rotate_point(point, component.rotation);
            [min[0] + point[0] * extent[0], min[1] + point[1] * extent[1]]
        });
        Some(Self::new(
            positions,
            if invalid {
                Self::ERROR_COLOR
            } else {
                Self::GATE_COLOR
            },
        ))
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
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: Arc<wgpu::Buffer>,
    vertex_capacity: usize,
    vertex_count: u32,
}

impl GridRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("editor.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("logic triangle pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("logic triangle pipeline"),
            layout: Some(&pipeline_layout),
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
        let vertex_capacity = 1;
        let vertex_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("logic triangle vertex buffer"),
            size: std::mem::size_of::<RenderVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));

        Self {
            pipeline,
            vertex_buffer,
            vertex_capacity,
            vertex_count: 0,
        }
    }

    fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &RenderFrame) {
        let triangles = frame_triangles(frame);
        let mut vertices = Vec::with_capacity(triangles.len() * 3);
        for triangle in triangles {
            vertices.extend(triangle.positions.map(|position| RenderVertex {
                position: world_to_clip(position, frame),
                fill_color: triangle.color,
            }));
        }

        if vertices.len() > self.vertex_capacity {
            self.vertex_capacity = vertices.len().next_power_of_two();
            self.vertex_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("logic triangle vertex buffer"),
                size: (self.vertex_capacity * std::mem::size_of::<RenderVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if !vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        }
        self.vertex_count = vertices.len() as u32;
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        if self.vertex_count == 0 {
            return;
        }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_count, 0..1);
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

fn frame_triangles(frame: &RenderFrame) -> Vec<DrawTriangle> {
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
    triangles.extend_from_slice(&frame.triangles);
    triangles
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
        triangles.extend(rectangle(
            [x - width * 0.5, top, x + width * 0.5, bottom],
            color,
        ));
        x += spacing;
    }
    let mut y = (top / spacing).floor() * spacing;
    while y <= bottom {
        triangles.extend(rectangle(
            [left, y - width * 0.5, right, y + width * 0.5],
            color,
        ));
        y += spacing;
    }
}

fn add_axis_lines(triangles: &mut Vec<DrawTriangle>, bounds: [f32; 4], width: f32) {
    let [left, top, right, bottom] = bounds;
    if left <= 0.0 && 0.0 <= right {
        triangles.extend(rectangle(
            [-width * 0.5, top, width * 0.5, bottom],
            AXIS_COLOR,
        ));
    }
    if top <= 0.0 && 0.0 <= bottom {
        triangles.extend(rectangle(
            [left, -width * 0.5, right, width * 0.5],
            AXIS_COLOR,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use logicgame::grid::{ComponentId, Point, Scale};

    #[test]
    fn wires_and_gates_are_emitted_as_filled_triangles() {
        let wire = Wire::new(Point::new(0, 0), Point::new(4, 0), Scale::ONE).unwrap();
        let wire_triangles = DrawTriangle::wire(wire, DrawTriangle::WIRE_COLOR);
        assert_eq!(wire_triangles.len(), 10);
        assert!(wire_triangles
            .iter()
            .all(|triangle| triangle.color == DrawTriangle::WIRE_COLOR));
        assert_eq!(wire_triangles[2].positions[0], [0.5, 0.5]);
        assert_eq!(wire_triangles[6].positions[0], [4.5, 0.5]);

        let gate = Component {
            id: ComponentId(0),
            position: Point::new(10, 20),
            rotation: Rotation::Right,
            kind: ComponentKind::Not { scale: Scale::ONE },
        };
        let gate_triangle = DrawTriangle::component(&gate, false).unwrap();
        let tip = gate_triangle.positions[2];
        assert!(tip[0] > gate_triangle.positions[0][0]);
        assert!(tip[0] > gate_triangle.positions[1][0]);
        assert_eq!(gate_triangle.color, DrawTriangle::GATE_COLOR);

        let highlight = DrawTriangle::component_highlight(&gate);
        assert_eq!(highlight.len(), 8);
        assert!(highlight
            .iter()
            .all(|triangle| triangle.color == DrawTriangle::HIGHLIGHT_COLOR));
    }

    #[test]
    fn one_x_grid_emits_lines_one_world_unit_apart() {
        let frame = RenderFrame {
            viewport_size: [80.0, 80.0],
            camera_center: [0.0, 0.0],
            zoom: 10.0,
            grid_scale: 1.0,
            triangles: Vec::new(),
        };
        let triangles = frame_triangles(&frame);

        let first_minor_x = triangles[2].positions[0][0];
        let second_minor_x = triangles[4].positions[0][0];
        assert!((second_minor_x - first_minor_x - 1.0).abs() < 0.0001);
        assert_eq!(triangles[2].color, MINOR_GRID_COLOR);
        assert_eq!(triangles[4].color, MINOR_GRID_COLOR);
    }
}
