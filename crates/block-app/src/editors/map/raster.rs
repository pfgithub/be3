use super::mvt::{Feature, GeometryKind, Layer, TagValue, Tile};

pub(super) const TILE_PIXELS: usize = 512;
const SUPERSAMPLE: usize = 2;
const CANVAS: usize = TILE_PIXELS * SUPERSAMPLE;

const LAND: [u8; 4] = [242, 239, 233, 255];
const WATER: [u8; 4] = [170, 211, 223, 255];
const BUILDING: [u8; 4] = [217, 208, 201, 255];
const SITE: [u8; 4] = [232, 229, 219, 255];
const PEDESTRIAN_AREA: [u8; 4] = [237, 237, 237, 255];
const BOUNDARY: [u8; 4] = [140, 110, 150, 170];
const PLACE_LABEL_COLOR: [u8; 3] = [60, 60, 60];
const BOUNDARY_LABEL_COLOR: [u8; 3] = [110, 100, 125];
const MAX_LABELS: usize = 80;

#[derive(Clone, Debug)]
pub(super) struct TileLabel {
    pub text: String,

    pub position: [f32; 2],
    pub font_size: f32,
    pub color: [u8; 3],
}

pub(super) struct TileRaster {
    pub pixels: Vec<u8>,
    pub labels: Vec<TileLabel>,
}

pub(super) fn rasterize(tile: &Tile, zoom: u8) -> TileRaster {
    let mut canvas = Canvas::new();
    let line_scale = line_width_scale(zoom);

    fill_layer(&mut canvas, tile.layer("ocean"), |_, _| Some(WATER));
    fill_layer(&mut canvas, tile.layer("land"), land_color);
    fill_layer(&mut canvas, tile.layer("sites"), |_, _| Some(SITE));
    fill_layer(&mut canvas, tile.layer("water_polygons"), |_, _| {
        Some(WATER)
    });
    stroke_layer(&mut canvas, tile.layer("water_lines"), |layer, feature| {
        let width = match kind(layer, feature) {
            "river" => 3.5,
            "canal" => 2.5,
            _ => 1.2,
        };
        Some((WATER, width * line_scale))
    });
    fill_layer(&mut canvas, tile.layer("pier_polygons"), |_, _| Some(LAND));
    stroke_layer(&mut canvas, tile.layer("pier_lines"), |_, _| {
        Some((LAND, 3.0))
    });
    fill_layer(
        &mut canvas,
        tile.layer("street_polygons"),
        |layer, feature| match kind(layer, feature) {
            "pedestrian" | "footway" => Some(PEDESTRIAN_AREA),
            _ => Some([255, 255, 255, 255]),
        },
    );
    streets(&mut canvas, tile.layer("streets"), line_scale);
    fill_layer(&mut canvas, tile.layer("buildings"), |_, _| Some(BUILDING));
    stroke_layer(&mut canvas, tile.layer("boundaries"), |layer, feature| {
        let admin_level = layer
            .tag(feature, "admin_level")
            .and_then(TagValue::as_i64)
            .unwrap_or(2);
        let width = if admin_level <= 2 { 2.2 } else { 1.2 };
        Some((BOUNDARY, width))
    });

    TileRaster {
        pixels: canvas.into_pixels(),
        labels: labels(tile),
    }
}

fn kind<'a>(layer: &'a Layer, feature: &Feature) -> &'a str {
    layer
        .tag(feature, "kind")
        .and_then(TagValue::as_str)
        .unwrap_or("")
}

fn land_color(layer: &Layer, feature: &Feature) -> Option<[u8; 4]> {
    match kind(layer, feature) {
        "forest" | "wood" => Some([157, 202, 138, 255]),
        "grass" | "grassland" | "meadow" | "garden" | "park" | "village_green"
        | "recreation_ground" | "cemetery" | "golf_course" | "miniature_golf" | "playground" => {
            Some([205, 235, 176, 255])
        }
        "heath" | "scrub" => Some([214, 217, 159, 255]),
        "farmland"
        | "orchard"
        | "vineyard"
        | "allotments"
        | "greenhouse_horticulture"
        | "plant_nursery"
        | "greenfield" => Some([238, 240, 213, 255]),
        "residential" | "commercial" | "retail" | "industrial" | "garages" | "railway"
        | "landfill" | "brownfield" | "quarry" => Some([224, 222, 219, 255]),
        "sand" | "beach" | "dune" => Some([245, 233, 198, 255]),
        "rock" | "bare_rock" | "scree" | "shingle" => Some([219, 217, 214, 255]),
        "glacier" => Some([221, 236, 240, 255]),
        "swamp" | "bog" | "string_bog" | "wet_meadow" | "marsh" => Some([211, 230, 219, 255]),
        _ => None,
    }
}

fn line_width_scale(zoom: u8) -> f32 {
    match zoom {
        0..=7 => 0.35,
        8..=9 => 0.45,
        10..=11 => 0.55,
        12..=13 => 0.75,
        _ => 1.0,
    }
}

fn street_style(kind: &str) -> Option<(u8, [u8; 4], f32)> {
    match kind {
        "motorway" => Some((8, [233, 144, 161, 255], 6.0)),
        "trunk" => Some((7, [250, 163, 141, 255], 5.5)),
        "primary" => Some((5, [252, 214, 164, 255], 5.5)),
        "secondary" => Some((4, [247, 250, 191, 255], 4.5)),
        "tertiary" => Some((3, [255, 255, 255, 255], 4.0)),
        "residential" | "unclassified" | "living_street" | "road" | "busway" => {
            Some((2, [255, 255, 255, 255], 3.0))
        }
        "service" => Some((1, [255, 255, 255, 255], 1.8)),
        "pedestrian" => Some((1, [237, 237, 237, 255], 2.0)),
        "track" => Some((0, [172, 148, 109, 255], 1.6)),
        "path" | "footway" | "steps" | "bridleway" => Some((0, [200, 140, 120, 255], 1.2)),
        "cycleway" => Some((0, [130, 130, 220, 255], 1.2)),
        "rail" => Some((6, [140, 140, 140, 255], 1.6)),
        "light_rail" | "subway" | "tram" | "narrow_gauge" | "funicular" | "monorail" => {
            Some((6, [150, 150, 150, 255], 1.4))
        }
        _ => None,
    }
}

fn streets(canvas: &mut Canvas, layer: Option<&Layer>, line_scale: f32) {
    let Some(layer) = layer else {
        return;
    };
    let mut styled: Vec<(u8, [u8; 4], f32, &Feature)> = layer
        .features
        .iter()
        .filter(|feature| feature.kind == GeometryKind::Line)
        .filter_map(|feature| {
            let (rank, color, width) = street_style(kind(layer, feature))?;
            Some((rank, color, width * line_scale, feature))
        })
        .collect();
    styled.sort_by_key(|(rank, ..)| *rank);
    for (_, color, width, feature) in styled {
        let paths = transformed(feature, layer.extent);
        canvas.stroke(&paths, width * SUPERSAMPLE as f32, color);
    }
}

fn fill_layer(
    canvas: &mut Canvas,
    layer: Option<&Layer>,
    style: impl Fn(&Layer, &Feature) -> Option<[u8; 4]>,
) {
    let Some(layer) = layer else {
        return;
    };
    for feature in &layer.features {
        if feature.kind != GeometryKind::Polygon {
            continue;
        }
        let Some(color) = style(layer, feature) else {
            continue;
        };
        let paths = transformed(feature, layer.extent);
        canvas.fill(&paths, color);
    }
}

fn stroke_layer(
    canvas: &mut Canvas,
    layer: Option<&Layer>,
    style: impl Fn(&Layer, &Feature) -> Option<([u8; 4], f32)>,
) {
    let Some(layer) = layer else {
        return;
    };
    for feature in &layer.features {
        if feature.kind != GeometryKind::Line {
            continue;
        }
        let Some((color, width)) = style(layer, feature) else {
            continue;
        };
        let paths = transformed(feature, layer.extent);
        canvas.stroke(&paths, width * SUPERSAMPLE as f32, color);
    }
}

fn transformed(feature: &Feature, extent: u32) -> Vec<Vec<[f32; 2]>> {
    let scale = CANVAS as f32 / extent as f32;
    feature
        .paths
        .iter()
        .map(|path| {
            path.iter()
                .map(|point| [point[0] * scale, point[1] * scale])
                .collect()
        })
        .collect()
}

fn labels(tile: &Tile) -> Vec<TileLabel> {
    let mut labels = Vec::new();
    if let Some(layer) = tile.layer("place_labels") {
        for feature in &layer.features {
            let Some(name) = layer.tag(feature, "name").and_then(TagValue::as_str) else {
                continue;
            };
            let font_size = match kind(layer, feature) {
                "capital" | "state_capital" => 15.0,
                "city" => 14.0,
                "town" => 12.0,
                "village" | "state" => 11.0,
                _ => 10.0,
            };
            push_label(
                &mut labels,
                layer,
                feature,
                name,
                font_size,
                PLACE_LABEL_COLOR,
            );
        }
    }
    if let Some(layer) = tile.layer("boundary_labels") {
        for feature in &layer.features {
            let Some(name) = layer.tag(feature, "name").and_then(TagValue::as_str) else {
                continue;
            };
            let admin_level = layer
                .tag(feature, "admin_level")
                .and_then(TagValue::as_i64)
                .unwrap_or(2);
            let font_size = if admin_level <= 2 { 13.0 } else { 11.0 };
            push_label(
                &mut labels,
                layer,
                feature,
                name,
                font_size,
                BOUNDARY_LABEL_COLOR,
            );
        }
    }
    labels.sort_by(|a, b| b.font_size.total_cmp(&a.font_size));
    labels.truncate(MAX_LABELS);
    labels
}

fn push_label(
    labels: &mut Vec<TileLabel>,
    layer: &Layer,
    feature: &Feature,
    name: &str,
    font_size: f32,
    color: [u8; 3],
) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    let Some(point) = feature.paths.first().and_then(|path| path.first()) else {
        return;
    };
    let scale = TILE_PIXELS as f32 / layer.extent as f32;
    let position = [point[0] * scale, point[1] * scale];
    let bounds = 0.0..TILE_PIXELS as f32;
    if !bounds.contains(&position[0]) || !bounds.contains(&position[1]) {
        return;
    }
    labels.push(TileLabel {
        text: name.to_owned(),
        position,
        font_size,
        color,
    });
}

struct Canvas {
    pixels: Vec<[u8; 4]>,
    crossings: Vec<f32>,
}

impl Canvas {
    fn new() -> Self {
        Self {
            pixels: vec![LAND; CANVAS * CANVAS],
            crossings: Vec::new(),
        }
    }

    fn fill<P: AsRef<[[f32; 2]]>>(&mut self, paths: &[P], color: [u8; 4]) {
        let mut min = [f32::MAX; 2];
        let mut max = [f32::MIN; 2];
        for path in paths {
            for point in path.as_ref() {
                for axis in 0..2 {
                    min[axis] = min[axis].min(point[axis]);
                    max[axis] = max[axis].max(point[axis]);
                }
            }
        }
        if min[0] >= CANVAS as f32 || max[0] <= 0.0 || min[1] >= CANVAS as f32 || max[1] <= 0.0 {
            return;
        }
        let rows = (min[1].floor().max(0.0) as usize)..(max[1].ceil().min(CANVAS as f32) as usize);
        let mut crossings = std::mem::take(&mut self.crossings);
        for row in rows {
            let sample_y = row as f32 + 0.5;
            crossings.clear();
            for path in paths {
                let path = path.as_ref();
                if path.len() < 3 {
                    continue;
                }
                for index in 0..path.len() {
                    let a = path[index];
                    let b = path[(index + 1) % path.len()];
                    if (a[1] <= sample_y) != (b[1] <= sample_y) {
                        let t = (sample_y - a[1]) / (b[1] - a[1]);
                        crossings.push(a[0] + t * (b[0] - a[0]));
                    }
                }
            }
            crossings.sort_unstable_by(f32::total_cmp);
            for span in crossings.chunks_exact(2) {
                let clamp = |edge: f32| ((edge - 0.5).ceil().max(0.0) as usize).min(CANVAS);
                let start = clamp(span[0]);
                let end = clamp(span[1]);
                for pixel in &mut self.pixels[row * CANVAS + start..row * CANVAS + end] {
                    blend(pixel, color);
                }
            }
        }
        self.crossings = crossings;
    }

    fn stroke<P: AsRef<[[f32; 2]]>>(&mut self, paths: &[P], width: f32, color: [u8; 4]) {
        let half = (width * 0.5).max(0.35);
        for path in paths {
            let path = path.as_ref();
            for segment in path.windows(2) {
                let [ax, ay] = segment[0];
                let [bx, by] = segment[1];
                let (dx, dy) = (bx - ax, by - ay);
                let length = (dx * dx + dy * dy).sqrt();
                if length < 1e-3 {
                    continue;
                }

                let (ex, ey) = (dx / length * half, dy / length * half);
                let (nx, ny) = (-ey, ex);
                let quad = [
                    [ax - ex + nx, ay - ey + ny],
                    [bx + ex + nx, by + ey + ny],
                    [bx + ex - nx, by + ey - ny],
                    [ax - ex - nx, ay - ey - ny],
                ];
                self.fill(&[quad], color);
            }
        }
    }

    fn into_pixels(self) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(TILE_PIXELS * TILE_PIXELS * 4);
        for y in 0..TILE_PIXELS {
            for x in 0..TILE_PIXELS {
                let mut sum = [0u32; 3];
                for sy in 0..SUPERSAMPLE {
                    for sx in 0..SUPERSAMPLE {
                        let sample =
                            self.pixels[(y * SUPERSAMPLE + sy) * CANVAS + x * SUPERSAMPLE + sx];
                        for channel in 0..3 {
                            sum[channel] += u32::from(sample[channel]);
                        }
                    }
                }
                let samples = (SUPERSAMPLE * SUPERSAMPLE) as u32;
                pixels.extend([
                    (sum[0] / samples) as u8,
                    (sum[1] / samples) as u8,
                    (sum[2] / samples) as u8,
                    255,
                ]);
            }
        }
        pixels
    }
}

fn blend(pixel: &mut [u8; 4], color: [u8; 4]) {
    let alpha = u32::from(color[3]);
    if alpha == 255 {
        *pixel = color;
        return;
    }
    for channel in 0..3 {
        let blended = u32::from(color[channel]) * alpha + u32::from(pixel[channel]) * (255 - alpha);
        pixel[channel] = (blended / 255) as u8;
    }
}

#[cfg(test)]
mod tests;
