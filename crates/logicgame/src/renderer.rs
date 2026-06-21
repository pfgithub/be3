use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::{self, wgpu};
use logicgame::grid::{
    Component, ComponentKind, ComponentLead, ComponentOrientation, ComponentSide,
    ConnectionDirection, ConnectionSlot, GridBounds, Orientation, Rotation, Scale, Wire,
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
    pub rays: Vec<DrawRay>,
    pub stubs: Vec<DrawStub>,
    pub value_triangles: Vec<DrawValueTriangle>,
    pub triangles: Vec<DrawTriangle>,
}

#[derive(Clone, Copy, Debug)]
pub struct DrawWire {
    wire: Wire,
    color: [f32; 4],
    value_index: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct DrawValueTriangle {
    positions: [[f32; 2]; 3],
    bit_coords: [f32; 3],
    scale: f32,
    color: [f32; 4],
    value_index: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct WireValue {
    low: u32,
    high: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct DrawRay {
    pub lead: ComponentLead,
    pub scale: f32,
    pub color: [f32; 4],
    pub value_index: u32,
}

impl DrawRay {
    pub fn from_component(
        component: &Component,
        color: [f32; 4],
        value_index: u32,
    ) -> Option<Self> {
        let lead = component.lead()?;
        let scale = component.kind.snap().get() as f32;
        Some(Self {
            lead,
            scale,
            color,
            value_index,
        })
    }
}

/// A short wire stub drawn extending outward from a port wired to a wire net,
/// so the connection reads as a wire entering the component. It runs through the
/// wire pipeline, so it shows the net's live on/off value and matches a regular
/// wire's width. The stub bridges the component boundary and the centerline of a
/// wire lying flush against that edge, which is half a cell away.
#[derive(Clone, Copy, Debug)]
pub struct DrawStub {
    center: [f32; 2],
    side: ComponentSide,
    scale: f32,
    color: [f32; 4],
    value_index: u32,
}

impl DrawStub {
    pub fn for_connection(
        component: &Component,
        connection: ConnectionSlot,
        color: [f32; 4],
        value_index: u32,
    ) -> Self {
        Self {
            center: connection_boundary_center(component, connection),
            side: connection.side,
            scale: connection.scale.get() as f32,
            color,
            value_index,
        }
    }
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

impl DrawValueTriangle {
    pub fn new(
        positions: [[f32; 2]; 3],
        bit_coords: [f32; 3],
        scale: f32,
        color: [f32; 4],
        value_index: u32,
    ) -> Self {
        Self {
            positions,
            bit_coords,
            scale,
            color,
            value_index,
        }
    }

    pub fn connection_marker(
        component: &Component,
        connection: ConnectionSlot,
        radius: f32,
        value_index: u32,
    ) -> Self {
        let marker = connection_marker(component, connection, radius)[0];
        let color = match connection.direction {
            ConnectionDirection::Input => DrawTriangle::INPUT_COLOR,
            ConnectionDirection::Output => DrawTriangle::OUTPUT_COLOR,
        };
        let bit_coords = connection_marker_bit_coords(connection, radius, marker.positions);
        Self::new(
            marker.positions,
            bit_coords,
            connection.scale.get() as f32,
            color,
            value_index,
        )
    }

    pub fn storage_state(component: &Component, value_index: u32) -> Vec<Self> {
        let Some(size) = component.size() else {
            return Vec::new();
        };
        let ComponentKind::Storage { scale, .. } = component.kind else {
            return Vec::new();
        };
        let min = [component.position.x as f32, component.position.y as f32];
        let extent = [size.width as f32, size.height as f32];
        let rect = [
            min[0] + extent[0] * 0.22,
            min[1] + extent[1] * 0.18,
            min[0] + extent[0] * 0.78,
            min[1] + extent[1] * 0.82,
        ];
        value_rectangle(
            rect,
            scale.get() as f32,
            DrawTriangle::WIRE_COLOR,
            value_index,
        )
        .to_vec()
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
    pub const WIRE_ENDPOINT_COLOR: [f32; 4] = [0.95, 0.86, 0.42, 1.0];
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
        triangles.extend(square(start, scale * 0.38, color));
        triangles.extend(square(end, scale * 0.38, color));
        triangles
    }

    pub fn wire_endpoints(wire: Wire) -> Vec<Self> {
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

        let mut triangles = Vec::with_capacity(4);
        triangles.extend(square(start, scale * 0.38, Self::WIRE_ENDPOINT_COLOR));
        triangles.extend(square(end, scale * 0.38, Self::WIRE_ENDPOINT_COLOR));
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
                    let point = transform_point(point, component.orientation);
                    [min[0] + point[0] * extent[0], min[1] + point[1] * extent[1]]
                });
                triangles.push(Self::new(positions, outline_color));
            }
            ComponentKind::MergerSplitter {
                input_scale,
                output_scale,
            } => {
                triangles.extend(merger_splitter_order_lines(
                    component,
                    input_scale,
                    output_scale,
                    outline_color,
                ));
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

fn transform_point(point: [f32; 2], orientation: ComponentOrientation) -> [f32; 2] {
    let mut point = rotate_point(point, orientation.rotation());
    if orientation.is_mirrored() {
        point[0] = 1.0 - point[0];
    }
    point
}

fn rectangle(rect: [f32; 4], color: [f32; 4]) -> [DrawTriangle; 2] {
    let [left, top, right, bottom] = rect;
    [
        DrawTriangle::new([[left, top], [right, top], [left, bottom]], color),
        DrawTriangle::new([[left, bottom], [right, top], [right, bottom]], color),
    ]
}

fn line_segment(start: [f32; 2], end: [f32; 2], radius: f32, color: [f32; 4]) -> [DrawTriangle; 2] {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return square(start, radius, color);
    }

    let normal = [-(dy / length) * radius, (dx / length) * radius];
    let a = [start[0] + normal[0], start[1] + normal[1]];
    let b = [start[0] - normal[0], start[1] - normal[1]];
    let c = [end[0] + normal[0], end[1] + normal[1]];
    let d = [end[0] - normal[0], end[1] - normal[1]];
    [
        DrawTriangle::new([a, c, b], color),
        DrawTriangle::new([b, c, d], color),
    ]
}

fn square(center: [f32; 2], radius: f32, color: [f32; 4]) -> [DrawTriangle; 2] {
    let [x, y] = center;
    rectangle([x - radius, y - radius, x + radius, y + radius], color)
}

fn value_rectangle(
    rect: [f32; 4],
    scale: f32,
    color: [f32; 4],
    value_index: u32,
) -> [DrawValueTriangle; 2] {
    let [left, top, right, bottom] = rect;
    [
        DrawValueTriangle::new(
            [[left, top], [right, top], [left, bottom]],
            [0.0, scale, 0.0],
            scale,
            color,
            value_index,
        ),
        DrawValueTriangle::new(
            [[left, bottom], [right, top], [right, bottom]],
            [0.0, scale, scale],
            scale,
            color,
            value_index,
        ),
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

fn connection_boundary_center(component: &Component, connection: ConnectionSlot) -> [f32; 2] {
    match connection.side {
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
    }
}

fn connection_marker(
    component: &Component,
    connection: ConnectionSlot,
    radius: f32,
) -> [DrawTriangle; 1] {
    let center = connection_boundary_center(component, connection);
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

fn connection_marker_bit_coords(
    connection: ConnectionSlot,
    radius: f32,
    positions: [[f32; 2]; 3],
) -> [f32; 3] {
    let scale = connection.scale.get() as f32;
    let axis = match connection.side {
        ComponentSide::Top | ComponentSide::Bottom => 0,
        ComponentSide::Right | ComponentSide::Left => 1,
    };
    let min = positions
        .iter()
        .map(|position| position[axis])
        .fold(f32::INFINITY, f32::min);
    positions.map(|position| ((position[axis] - min) / (radius * 2.0)) * scale)
}

fn merger_splitter_order_lines(
    component: &Component,
    input_scale: Scale,
    output_scale: Scale,
    color: [f32; 4],
) -> Vec<DrawTriangle> {
    let large_scale = input_scale.max(output_scale);
    let small_scale = input_scale.min(output_scale);
    let slots = component.connection_slots();
    let Some(large_slot) = slots.iter().copied().find(|slot| slot.scale == large_scale) else {
        return Vec::new();
    };
    let mut small_slots = slots
        .into_iter()
        .filter(|slot| slot.scale == small_scale && slot.id != large_slot.id)
        .collect::<Vec<_>>();
    if small_slots.is_empty() {
        small_slots.push(large_slot);
    }
    small_slots.sort_by_key(|slot| slot.id);

    let Some(size) = component.size() else {
        return Vec::new();
    };
    let inset = size.width.min(size.height) as f32 * 0.3;
    let radius = (small_scale.get() as f32 * 0.045).clamp(0.035, 0.14);
    let chunk_width = small_scale.get() as f32;
    let mut triangles = Vec::with_capacity(small_slots.len() * 2);

    for (index, small_slot) in small_slots.into_iter().enumerate() {
        let chunk_center = large_slot.start as f32 + (index as f32 + 0.5) * chunk_width;
        let large_point = slot_interior_point(component, large_slot, chunk_center, inset);
        let small_point = slot_interior_point(
            component,
            small_slot,
            (small_slot.start + small_slot.end) as f32 * 0.5,
            inset,
        );
        triangles.extend(line_segment(large_point, small_point, radius, color));
    }

    triangles
}

fn slot_interior_point(
    component: &Component,
    slot: ConnectionSlot,
    coordinate: f32,
    inset: f32,
) -> [f32; 2] {
    let Some(size) = component.size() else {
        return connection_boundary_center(component, slot);
    };
    let left = component.position.x as f32;
    let top = component.position.y as f32;
    let right = left + size.width as f32;
    let bottom = top + size.height as f32;
    match slot.side {
        ComponentSide::Top => [coordinate, top + inset],
        ComponentSide::Right => [right - inset, coordinate],
        ComponentSide::Bottom => [coordinate, bottom - inset],
        ComponentSide::Left => [left + inset, coordinate],
    }
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
    value_vertex_buffer: Arc<wgpu::Buffer>,
    wire_value_buffer: Arc<wgpu::Buffer>,
    background_vertex_capacity: usize,
    vertex_capacity: usize,
    wire_vertex_capacity: usize,
    value_vertex_capacity: usize,
    wire_value_capacity: usize,
    background_vertex_count: u32,
    vertex_count: u32,
    wire_vertex_count: u32,
    value_vertex_count: u32,
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
        let value_vertex_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("logic value vertex buffer"),
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
            value_vertex_buffer,
            wire_value_buffer,
            background_vertex_capacity: vertex_capacity,
            vertex_capacity,
            wire_vertex_capacity: vertex_capacity,
            value_vertex_capacity: vertex_capacity,
            wire_value_capacity: vertex_capacity,
            background_vertex_count: 0,
            vertex_count: 0,
            wire_vertex_count: 0,
            value_vertex_count: 0,
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

        let mut wire_verts = wire_vertices(&frame.wires, frame);
        wire_verts.extend(ray_vertices(&frame.rays, frame));
        wire_verts.extend(stub_vertices(&frame.stubs, frame));
        prepare_vertex_buffer(
            device,
            queue,
            "logic wire vertex buffer",
            &wire_verts,
            &mut self.wire_vertex_buffer,
            &mut self.wire_vertex_capacity,
        );
        self.wire_vertex_count = wire_verts.len() as u32;

        let value_verts = value_triangle_vertices(&frame.value_triangles, frame);
        prepare_vertex_buffer(
            device,
            queue,
            "logic value vertex buffer",
            &value_verts,
            &mut self.value_vertex_buffer,
            &mut self.value_vertex_capacity,
        );
        self.value_vertex_count = value_verts.len() as u32;

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
        if self.value_vertex_count > 0 {
            render_pass.set_pipeline(&self.wire_pipeline);
            render_pass.set_bind_group(0, &self.wire_value_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.value_vertex_buffer.slice(..));
            render_pass.draw(0..self.value_vertex_count, 0..1);
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
        let top_left = wire_vertex([left, top], 0.0, scale, draw.color, draw.value_index, frame);
        let top_right = wire_vertex(
            [right, top],
            if horizontal { 0.0 } else { scale },
            scale,
            draw.color,
            draw.value_index,
            frame,
        );
        let bottom_left = wire_vertex(
            [left, bottom],
            if horizontal { scale } else { 0.0 },
            scale,
            draw.color,
            draw.value_index,
            frame,
        );
        let bottom_right = wire_vertex(
            [right, bottom],
            scale,
            scale,
            draw.color,
            draw.value_index,
            frame,
        );
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

fn ray_vertices(rays: &[DrawRay], frame: &RenderFrame) -> Vec<WireVertex> {
    let half_width = frame.viewport_size[0] / frame.zoom * 0.5;
    let half_height = frame.viewport_size[1] / frame.zoom * 0.5;
    let vp_left = frame.camera_center[0] - half_width;
    let vp_top = frame.camera_center[1] - half_height;
    let vp_right = frame.camera_center[0] + half_width;
    let vp_bottom = frame.camera_center[1] + half_height;

    let mut vertices = Vec::with_capacity(rays.len() * 6);
    for ray in rays {
        let ComponentLead {
            side,
            axis,
            start,
            end,
        } = ray.lead;
        let (left, top, right, bottom) = match side {
            ComponentSide::Top => (start as f32, vp_top, end as f32, axis as f32),
            ComponentSide::Bottom => (start as f32, axis as f32, end as f32, vp_bottom),
            ComponentSide::Left => (vp_left, start as f32, axis as f32, end as f32),
            ComponentSide::Right => (axis as f32, start as f32, vp_right, end as f32),
        };
        if left >= right || top >= bottom {
            continue;
        }
        let scale = ray.scale;
        // bit_coord varies along the band's width (perpendicular to the ray direction):
        // Top/Bottom rays: band runs in x, bit 0 at left, bit scale at right
        // Left/Right rays: band runs in y, bit 0 at top, bit scale at bottom
        let (tl_bit, tr_bit, bl_bit, br_bit) = match side {
            ComponentSide::Top | ComponentSide::Bottom => (0.0, scale, 0.0, scale),
            ComponentSide::Left | ComponentSide::Right => (0.0, 0.0, scale, scale),
        };
        let tl = wire_vertex(
            [left, top],
            tl_bit,
            scale,
            ray.color,
            ray.value_index,
            frame,
        );
        let tr = wire_vertex(
            [right, top],
            tr_bit,
            scale,
            ray.color,
            ray.value_index,
            frame,
        );
        let bl = wire_vertex(
            [left, bottom],
            bl_bit,
            scale,
            ray.color,
            ray.value_index,
            frame,
        );
        let br = wire_vertex(
            [right, bottom],
            br_bit,
            scale,
            ray.color,
            ray.value_index,
            frame,
        );
        vertices.extend([tl, tr, bl, bl, tr, br]);
    }
    vertices
}

fn stub_vertices(stubs: &[DrawStub], frame: &RenderFrame) -> Vec<WireVertex> {
    let mut vertices = Vec::with_capacity(stubs.len() * 6);
    for stub in stubs {
        let scale = stub.scale;
        let half_thickness = (scale * 0.36).max(0.08);
        let length = scale * 0.5;
        let [cx, cy] = stub.center;
        let (left, top, right, bottom) = match stub.side {
            ComponentSide::Top => (cx - half_thickness, cy - length, cx + half_thickness, cy),
            ComponentSide::Bottom => (cx - half_thickness, cy, cx + half_thickness, cy + length),
            ComponentSide::Left => (cx - length, cy - half_thickness, cx, cy + half_thickness),
            ComponentSide::Right => (cx, cy - half_thickness, cx + length, cy + half_thickness),
        };
        // bit_coord runs across the stub's width, matching a wire of the same
        // orientation: vertical stubs (Top/Bottom) vary the bit in x, horizontal
        // stubs (Left/Right) vary it in y.
        let (tl_bit, tr_bit, bl_bit, br_bit) = match stub.side {
            ComponentSide::Top | ComponentSide::Bottom => (0.0, scale, 0.0, scale),
            ComponentSide::Left | ComponentSide::Right => (0.0, 0.0, scale, scale),
        };
        let tl = wire_vertex(
            [left, top],
            tl_bit,
            scale,
            stub.color,
            stub.value_index,
            frame,
        );
        let tr = wire_vertex(
            [right, top],
            tr_bit,
            scale,
            stub.color,
            stub.value_index,
            frame,
        );
        let bl = wire_vertex(
            [left, bottom],
            bl_bit,
            scale,
            stub.color,
            stub.value_index,
            frame,
        );
        let br = wire_vertex(
            [right, bottom],
            br_bit,
            scale,
            stub.color,
            stub.value_index,
            frame,
        );
        vertices.extend([tl, tr, bl, bl, tr, br]);
    }
    vertices
}

fn value_triangle_vertices(
    triangles: &[DrawValueTriangle],
    frame: &RenderFrame,
) -> Vec<WireVertex> {
    let mut vertices = Vec::with_capacity(triangles.len() * 3);
    for triangle in triangles {
        for (position, bit_coord) in triangle.positions.into_iter().zip(triangle.bit_coords) {
            vertices.push(wire_vertex(
                position,
                bit_coord,
                triangle.scale,
                triangle.color,
                triangle.value_index,
                frame,
            ));
        }
    }
    vertices
}

fn wire_vertex(
    position: [f32; 2],
    bit_coord: f32,
    scale: f32,
    color: [f32; 4],
    value_index: u32,
    frame: &RenderFrame,
) -> WireVertex {
    WireVertex {
        position: world_to_clip(position, frame),
        bit_coord,
        scale,
        value_index,
        fill_color: color,
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
mod tests;
