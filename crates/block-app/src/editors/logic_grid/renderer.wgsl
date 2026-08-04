struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) fill_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) fill_color: vec4<f32>,
};

struct WireVertexInput {
    @location(0) position: vec2<f32>,
    @location(2) bit_coord: f32,
    @location(3) scale: f32,
    @location(4) value_index: u32,
    @location(5) fill_color: vec4<f32>,
};

struct WireVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) bit_coord: f32,
    @location(1) scale: f32,
    @location(2) fill_color: vec4<f32>,
    @location(3) @interpolate(flat) value_index: u32,
};

struct WireValue {
    low: u32,
    high: u32,
};

@group(0) @binding(0)
var<storage, read> wire_values: array<WireValue>;

@vertex
fn vertex(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.fill_color = input.fill_color;
    return output;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.fill_color;
}

@vertex
fn wire_vertex(input: WireVertexInput) -> WireVertexOutput {
    var output: WireVertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.bit_coord = input.bit_coord;
    output.scale = input.scale;
    output.value_index = input.value_index;
    output.fill_color = input.fill_color;
    return output;
}

@fragment
fn wire_fragment(input: WireVertexOutput) -> @location(0) vec4<f32> {
    let scale = max(input.scale, 1.0);
    let bit = min(u32(floor(clamp(input.bit_coord, 0.0, scale - 0.0001))), 63u);
    let value = wire_values[input.value_index];
    let word = select(value.low, value.high, bit >= 32u);
    let shift = bit & 31u;
    let is_on = (word & (1u << shift)) != 0u;

    let off_color = vec4<f32>(input.fill_color.rgb * 0.28, input.fill_color.a);
    var color = select(off_color, input.fill_color, is_on);
    if scale > 1.0 {
        let within_bit = fract(input.bit_coord);
        if within_bit < 0.08 || within_bit > 0.92 {
            color = vec4<f32>(color.rgb * 0.48, color.a);
        }
    }
    return color;
}
