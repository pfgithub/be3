struct BlitOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opacity: f32,
};

struct Region {
    offset: vec2<f32>,
    scale: vec2<f32>,
    corner_0: vec2<f32>,
    corner_1: vec2<f32>,
    corner_2: vec2<f32>,
    corner_3: vec2<f32>,
    opacity: f32,
};

@group(0) @binding(2)
var<uniform> region: Region;

@vertex
fn blit_vertex(@builtin(vertex_index) index: u32) -> BlitOutput {
    var order = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    var uvs = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    var corners = array<vec2<f32>, 4>(
        region.corner_0,
        region.corner_1,
        region.corner_2,
        region.corner_3,
    );
    let corner = order[index];
    var output: BlitOutput;
    output.position = vec4<f32>(corners[corner], 0.0, 1.0);
    output.uv = region.offset + uvs[corner] * region.scale;
    output.opacity = region.opacity;
    return output;
}

@group(0) @binding(0)
var counter_texture: texture_2d<f32>;
@group(0) @binding(1)
var counter_sampler: sampler;

@fragment
fn blit_fragment(input: BlitOutput) -> @location(0) vec4<f32> {
    let color = textureSample(counter_texture, counter_sampler, input.uv);
    return vec4<f32>(color.rgb, color.a * input.opacity);
}
