struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@group(0) @binding(0)
var glyph_atlas: texture_2d<f32>;

@group(0) @binding(1)
var glyph_sampler: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.tex_coord = input.tex_coord;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    var coverage = 1.0;
    if input.tex_coord.x >= 0.0 {
        coverage = textureSample(glyph_atlas, glyph_sampler, input.tex_coord).r;
    }
    return vec4<f32>(input.color.rgb, input.color.a * coverage);
}
