struct Uniforms {
    time: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

const TAU: f32 = 6.28318530718;

@vertex
fn vertex_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let angle = uniforms.time * 0.8 + f32(index) * (TAU / 3.0);
    let hue = uniforms.time * 0.3 + f32(index) / 3.0;

    var output: VertexOutput;
    output.position = vec4<f32>(cos(angle) * 0.6, sin(angle) * 0.6, 0.0, 1.0);
    output.color = vec3<f32>(
        0.5 + 0.5 * sin(hue * TAU),
        0.5 + 0.5 * sin(hue * TAU + 2.094),
        0.5 + 0.5 * sin(hue * TAU + 4.188),
    );
    return output;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
