struct Uniforms {
    screen: vec2<f32>,
    padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct Instance {
    @location(0) rect: vec4<f32>,
    @location(1) clip: vec4<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) params: vec4<f32>,
};

struct Fragment {
    @builtin(position) position: vec4<f32>,
    @location(0) point: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) rect: vec4<f32>,
    @location(4) clip: vec4<f32>,
    @location(5) params: vec4<f32>,
};

fn corner(index: u32) -> vec2<f32> {
    let right = index == 1u || index == 4u || index == 5u;
    let bottom = index == 2u || index == 3u || index == 5u;
    return vec2<f32>(select(0.0, 1.0, right), select(0.0, 1.0, bottom));
}

fn rounded_distance(point: vec2<f32>, extent: vec2<f32>, radius: f32) -> f32 {
    let limit = min(radius, min(extent.x, extent.y));
    let offset = abs(point) - extent + vec2<f32>(limit);
    return min(max(offset.x, offset.y), 0.0) + length(max(offset, vec2<f32>(0.0))) - limit;
}

@vertex
fn vertex(@builtin(vertex_index) index: u32, instance: Instance) -> Fragment {
    let is_glyph = instance.params.z > 0.5;
    let bleed = select(1.0, 0.0, is_glyph);
    let low = instance.rect.xy - vec2<f32>(bleed);
    let high = instance.rect.zw + vec2<f32>(bleed);
    let weight = corner(index);
    let point = mix(low, high, weight);

    var fragment: Fragment;
    fragment.position = vec4<f32>(
        point / uniforms.screen * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0),
        0.0,
        1.0,
    );
    fragment.point = point;
    fragment.uv = mix(instance.uv.xy, instance.uv.zw, weight);
    fragment.color = instance.color;
    fragment.rect = instance.rect;
    fragment.clip = instance.clip;
    fragment.params = instance.params;
    return fragment;
}

@fragment
fn fragment(input: Fragment) -> @location(0) vec4<f32> {
    if input.point.x < input.clip.x
        || input.point.y < input.clip.y
        || input.point.x > input.clip.z
        || input.point.y > input.clip.w {
        discard;
    }

    var coverage = 1.0;
    if input.params.z > 0.5 {
        coverage = textureSample(atlas, atlas_sampler, input.uv).r;
    } else {
        let center = (input.rect.xy + input.rect.zw) * 0.5;
        let extent = (input.rect.zw - input.rect.xy) * 0.5;
        var distance = rounded_distance(input.point - center, extent, input.params.x);
        let width = input.params.y;
        if width > 0.0 {
            distance = abs(distance + width * 0.5) - width * 0.5;
        }
        coverage = clamp(0.5 - distance, 0.0, 1.0);
    }

    return vec4<f32>(input.color.rgb, input.color.a * coverage);
}
