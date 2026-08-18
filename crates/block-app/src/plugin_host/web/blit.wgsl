struct BlitOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn blit_vertex(@builtin(vertex_index) index: u32) -> BlitOutput {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let corner = corners[index];
    var output: BlitOutput;
    output.position = vec4<f32>(corner, 0.0, 1.0);
    output.uv = vec2<f32>((corner.x + 1.0) * 0.5, (1.0 - corner.y) * 0.5);
    return output;
}

@group(0) @binding(0)
var plugin_demo_texture: texture_2d<f32>;
@group(0) @binding(1)
var plugin_demo_sampler: sampler;

@fragment
fn blit_fragment(input: BlitOutput) -> @location(0) vec4<f32> {
    return textureSample(plugin_demo_texture, plugin_demo_sampler, input.uv);
}
