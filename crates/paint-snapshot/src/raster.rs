use image::RgbaImage;

use crate::format::{Content, Frame, Snapshot, Texture};

pub fn render(snapshot: &Snapshot, frame: usize) -> Result<RgbaImage, String> {
    let frame: &Frame = snapshot.frame(frame)?;
    let [width, height] = frame.size;
    let mut canvas = RgbaImage::from_pixel(
        width.max(1),
        height.max(1),
        image::Rgba(opaque(frame.background)),
    );

    let mut textures = std::collections::HashMap::new();
    for (key, texture) in &snapshot.textures {
        textures.insert(*key, Sampler::new(texture)?);
    }

    let scale = frame.pixels_per_point;
    for primitive in &frame.primitives {
        let clip = scaled(primitive.clip, scale, &canvas);
        match &primitive.content {
            Content::Mesh(triangles) => {
                for triangle in triangles {
                    let sampler = textures
                        .get(&triangle.texture)
                        .ok_or("the snapshot is missing a texture the painting uses")?;
                    fill(&mut canvas, clip, triangle.corners, scale, sampler);
                }
            }
            Content::Callback(rect) => {
                let rect = scaled(*rect, scale, &canvas);
                outline(&mut canvas, rect);
            }
        }
    }
    Ok(canvas)
}

struct Sampler {
    size: [u32; 2],
    pixels: Vec<[u8; 4]>,
}

impl Sampler {
    fn new(texture: &Texture) -> Result<Self, String> {
        Ok(Self {
            size: texture.size,
            pixels: texture.pixels()?,
        })
    }

    fn sample(&self, uv: [f32; 2]) -> [f32; 4] {
        let x = uv[0] * self.size[0] as f32 - 0.5;
        let y = uv[1] * self.size[1] as f32 - 0.5;
        let left = x.floor();
        let top = y.floor();
        let fraction_x = x - left;
        let fraction_y = y - top;
        let mut total = [0.0; 4];
        for (offset_y, weight_y) in [(0.0, 1.0 - fraction_y), (1.0, fraction_y)] {
            for (offset_x, weight_x) in [(0.0, 1.0 - fraction_x), (1.0, fraction_x)] {
                let weight = weight_x * weight_y;
                if weight == 0.0 {
                    continue;
                }
                let texel = self.texel(left + offset_x, top + offset_y);
                for (total, texel) in total.iter_mut().zip(texel) {
                    *total += texel as f32 * weight;
                }
            }
        }
        total
    }

    fn texel(&self, x: f32, y: f32) -> [u8; 4] {
        let x = (x.max(0.0) as u32).min(self.size[0].saturating_sub(1));
        let y = (y.max(0.0) as u32).min(self.size[1].saturating_sub(1));
        self.pixels
            .get((y * self.size[0] + x) as usize)
            .copied()
            .unwrap_or([0, 0, 0, 0])
    }
}

fn fill(
    canvas: &mut RgbaImage,
    clip: [i64; 4],
    corners: [crate::format::Vertex; 3],
    scale: f32,
    sampler: &Sampler,
) {
    let mut points = corners.map(|vertex| [vertex.pos[0] * scale, vertex.pos[1] * scale]);
    let mut corners = corners;
    if edge(points[0], points[1], points[2]) < 0.0 {
        points.swap(1, 2);
        corners.swap(1, 2);
    }
    let area = edge(points[0], points[1], points[2]);
    if area <= 0.0 {
        return;
    }

    let min_x = points.iter().map(|point| point[0]).fold(f32::MAX, f32::min);
    let max_x = points.iter().map(|point| point[0]).fold(f32::MIN, f32::max);
    let min_y = points.iter().map(|point| point[1]).fold(f32::MAX, f32::min);
    let max_y = points.iter().map(|point| point[1]).fold(f32::MIN, f32::max);
    let left = (min_x.floor() as i64).max(clip[0]);
    let right = (max_x.ceil() as i64)
        .min(clip[2])
        .min(canvas.width() as i64);
    let top = (min_y.floor() as i64).max(clip[1]);
    let bottom = (max_y.ceil() as i64)
        .min(clip[3])
        .min(canvas.height() as i64);

    let edges = [
        [points[1], points[2]],
        [points[2], points[0]],
        [points[0], points[1]],
    ];

    for y in top.max(0)..bottom {
        for x in left.max(0)..right {
            let point = [x as f32 + 0.5, y as f32 + 0.5];
            let weights = [
                edge(points[1], points[2], point),
                edge(points[2], points[0], point),
                edge(points[0], points[1], point),
            ];
            if weights
                .iter()
                .zip(edges)
                .any(|(weight, edge)| *weight < 0.0 || (*weight == 0.0 && !is_top_left(edge)))
            {
                continue;
            }

            let weights = weights.map(|weight| weight / area);
            let mut uv = [0.0; 2];
            let mut color = [0.0; 4];
            for (weight, corner) in weights.iter().zip(corners) {
                uv[0] += corner.uv[0] * weight;
                uv[1] += corner.uv[1] * weight;
                for (color, corner) in color.iter_mut().zip(corner.color) {
                    *color += corner as f32 * weight;
                }
            }

            let texel = sampler.sample(uv);
            let source = [
                texel[0] * color[0] / 255.0,
                texel[1] * color[1] / 255.0,
                texel[2] * color[2] / 255.0,
                texel[3] * color[3] / 255.0,
            ];
            let target = canvas.get_pixel_mut(x as u32, y as u32);
            let inverse = 1.0 - source[3] / 255.0;
            for (target, source) in target.0.iter_mut().zip(source) {
                let blended = source + *target as f32 * inverse;
                *target = blended.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

fn is_top_left(edge: [[f32; 2]; 2]) -> bool {
    let [start, end] = edge;
    (start[1] == end[1] && end[0] < start[0]) || end[1] < start[1]
}

fn edge(a: [f32; 2], b: [f32; 2], point: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (point[1] - a[1]) - (b[1] - a[1]) * (point[0] - a[0])
}

fn scaled(rect: [f32; 4], scale: f32, canvas: &RgbaImage) -> [i64; 4] {
    [
        (rect[0] * scale).floor() as i64,
        (rect[1] * scale).floor() as i64,
        (rect[2] * scale).ceil().min(canvas.width() as f32) as i64,
        (rect[3] * scale).ceil().min(canvas.height() as f32) as i64,
    ]
}

fn outline(canvas: &mut RgbaImage, rect: [i64; 4]) {
    let colour = image::Rgba([255, 0, 255, 255]);
    for x in rect[0].max(0)..rect[2].min(canvas.width() as i64) {
        for y in [
            rect[1].max(0),
            (rect[3] - 1).min(canvas.height() as i64 - 1),
        ] {
            if y >= 0 && y < canvas.height() as i64 {
                canvas.put_pixel(x as u32, y as u32, colour);
            }
        }
    }
    for y in rect[1].max(0)..rect[3].min(canvas.height() as i64) {
        for x in [rect[0].max(0), (rect[2] - 1).min(canvas.width() as i64 - 1)] {
            if x >= 0 && x < canvas.width() as i64 {
                canvas.put_pixel(x as u32, y as u32, colour);
            }
        }
    }
}

fn opaque(color: [u8; 4]) -> [u8; 4] {
    [color[0], color[1], color[2], 255]
}
