struct SceneUniform {
    view_projection: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> scene: SceneUniform;

struct SceneVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
};

struct SceneVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn scene_vertex(input: SceneVertexInput) -> SceneVertexOutput {
    var output: SceneVertexOutput;
    output.position = scene.view_projection * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    return output;
}

@fragment
fn scene_fragment(input: SceneVertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
