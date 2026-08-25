struct Punch {
    bounds: vec4<f32>,
    radius: f32,
    padding: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> punch: Punch;

@vertex
fn punch_vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    var points = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(points[index], 0.0, 1.0);
}

@fragment
fn punch_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let center = (punch.bounds.xy + punch.bounds.zw) * 0.5;
    let half_size = max((punch.bounds.zw - punch.bounds.xy) * 0.5, vec2<f32>(0.0, 0.0));
    let radius = min(punch.radius, min(half_size.x, half_size.y));
    let corner = abs(position.xy - center) - (half_size - vec2<f32>(radius, radius));
    let outside = length(max(corner, vec2<f32>(0.0, 0.0)));
    let inside = min(max(corner.x, corner.y), 0.0);
    let distance = outside + inside - radius;
    let coverage = clamp(0.5 - distance, 0.0, 1.0);
    return vec4<f32>(0.0, 0.0, 0.0, coverage);
}
