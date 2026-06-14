struct GridUniform {
    viewport_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    grid_scale: f32,
    _padding: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> grid: GridUniform;

struct GridVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) screen: vec2<f32>,
};

@vertex
fn grid_vs(@builtin(vertex_index) vertex_index: u32) -> GridVertexOutput {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let position = positions[vertex_index];
    var output: GridVertexOutput;
    output.position = vec4<f32>(position, 0.0, 1.0);
    output.screen = (position * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5)) * grid.viewport_size;
    return output;
}

fn grid_line(distance_to_line: vec2<f32>, width: f32) -> f32 {
    let pixel_distance = distance_to_line * grid.zoom;
    return 1.0 - smoothstep(width, width + 1.0, min(pixel_distance.x, pixel_distance.y));
}

@fragment
fn grid_fs(input: GridVertexOutput) -> @location(0) vec4<f32> {
    let world = grid.camera_center + (input.screen - grid.viewport_size * 0.5) / grid.zoom;
    let minor_distance =
        abs(fract(world / grid.grid_scale + 0.5) - 0.5) * grid.grid_scale;
    let major_scale = grid.grid_scale * 8.0;
    let major_distance = abs(fract(world / major_scale + 0.5) - 0.5) * major_scale;
    let axis_distance = abs(world);

    let minor = grid_line(minor_distance, 0.45);
    let major = grid_line(major_distance, 0.8);
    let axis = grid_line(axis_distance, 1.25);

    let background = vec3<f32>(0.035, 0.043, 0.055);
    var color = mix(background, vec3<f32>(0.10, 0.12, 0.15), minor);
    color = mix(color, vec3<f32>(0.18, 0.21, 0.26), major);
    color = mix(color, vec3<f32>(0.35, 0.39, 0.48), axis);
    return vec4<f32>(color, 1.0);
}

struct ShapeVertexInput {
    @location(0) rect: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) kind: u32,
    @location(3) rotation: u32,
};

struct ShapeVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    @location(2) @interpolate(flat) kind: u32,
    @location(3) @interpolate(flat) rotation: u32,
};

@vertex
fn shape_vs(
    input: ShapeVertexInput,
    @builtin(vertex_index) vertex_index: u32,
) -> ShapeVertexOutput {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let uv = corners[vertex_index];
    let world = mix(input.rect.xy, input.rect.zw, uv);
    let screen = (world - grid.camera_center) * grid.zoom + grid.viewport_size * 0.5;
    let clip = vec2<f32>(
        screen.x / grid.viewport_size.x * 2.0 - 1.0,
        1.0 - screen.y / grid.viewport_size.y * 2.0,
    );

    var output: ShapeVertexOutput;
    output.position = vec4<f32>(clip, 0.0, 1.0);
    output.uv = uv;
    output.color = input.color;
    output.kind = input.kind;
    output.rotation = input.rotation;
    return output;
}

fn canonical_gate_uv(uv: vec2<f32>, rotation: u32) -> vec2<f32> {
    switch rotation {
        case 0u: { return uv; }
        case 1u: { return vec2<f32>(uv.y, 1.0 - uv.x); }
        case 2u: { return vec2<f32>(1.0 - uv.x, 1.0 - uv.y); }
        default: { return vec2<f32>(1.0 - uv.y, uv.x); }
    }
}

fn segment_distance(point: vec2<f32>, start: vec2<f32>, end: vec2<f32>) -> f32 {
    let direction = end - start;
    let t = clamp(dot(point - start, direction) / dot(direction, direction), 0.0, 1.0);
    return length(point - (start + direction * t));
}

@fragment
fn shape_fs(input: ShapeVertexOutput) -> @location(0) vec4<f32> {
    if input.kind == 0u {
        return input.color;
    }

    let uv = canonical_gate_uv(input.uv, input.rotation);
    let a = vec2<f32>(0.12, 0.82);
    let b = vec2<f32>(0.88, 0.82);
    let tip = vec2<f32>(0.5, 0.24);
    let bubble_center = vec2<f32>(0.5, 0.13);
    let stroke = 0.045;
    let triangle = min(
        segment_distance(uv, a, b),
        min(segment_distance(uv, a, tip), segment_distance(uv, b, tip)),
    );
    let bubble = abs(length(uv - bubble_center) - 0.075);
    let input_stem = segment_distance(uv, vec2<f32>(0.5, 0.82), vec2<f32>(0.5, 1.0));
    let output_stem = segment_distance(uv, vec2<f32>(0.5, 0.0), vec2<f32>(0.5, 0.055));
    let distance = min(min(triangle, bubble), min(input_stem, output_stem));
    let alpha = 1.0 - smoothstep(stroke, stroke + 0.012, distance);
    if alpha <= 0.0 {
        discard;
    }
    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}
