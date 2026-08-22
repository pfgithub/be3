use std::collections::BTreeSet;

use block_client::blocks::pixel_art::PixelColor;

pub const MAX_BRUSH_SIZE: u16 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelTool {
    Pencil,
    Eraser,
    Fill,
    Eyedropper,
    ReplaceColor,
    Line,
    Rectangle,
    Ellipse,
}

impl PixelTool {
    pub const fn is_drawing(self) -> bool {
        matches!(
            self,
            Self::Pencil | Self::Eraser | Self::Line | Self::Rectangle | Self::Ellipse
        )
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Pencil => "Pencil",
            Self::Eraser => "Eraser",
            Self::Fill => "Fill",
            Self::Eyedropper => "Eyedropper",
            Self::ReplaceColor => "Replace Color",
            Self::Line => "Line",
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrushShape {
    Square,
    Circle,
}

#[derive(Clone, Debug)]
pub struct ActiveDrawing {
    pub tool: PixelTool,
    pub start: (u16, u16),
    pub end: (u16, u16),
    pub path: Vec<(u16, u16)>,
}

impl ActiveDrawing {
    pub fn new(tool: PixelTool, pixel: (u16, u16)) -> Self {
        Self {
            tool,
            start: pixel,
            end: pixel,
            path: vec![pixel],
        }
    }

    pub fn extend(&mut self, pixel: (u16, u16)) {
        if self.end == pixel {
            return;
        }
        if matches!(self.tool, PixelTool::Pencil | PixelTool::Eraser) {
            self.path
                .extend(pixels_on_line(self.end, pixel).into_iter().skip(1));
        }
        self.end = pixel;
    }
}

#[derive(Clone, Debug)]
pub struct CommittedPreview {
    pub pixels: Vec<(u16, u16)>,
    pub color: PixelColor,
    pub frames_remaining: u8,
}

/// Everything about the brush that turns a gesture into pixels.
#[derive(Clone, Copy, Debug)]
pub struct Brush {
    pub size: u16,
    pub shape: BrushShape,
    pub filled: bool,
    pub mirror_horizontal: bool,
    pub mirror_vertical: bool,
    pub constrained: bool,
}

pub fn rasterize_drawing(
    drawing: &ActiveDrawing,
    width: u16,
    height: u16,
    brush: Brush,
) -> Vec<(u16, u16)> {
    let end = if brush.constrained
        && matches!(
            drawing.tool,
            PixelTool::Line | PixelTool::Rectangle | PixelTool::Ellipse
        ) {
        constrained_endpoint(drawing.start, drawing.end, drawing.tool, width, height)
    } else {
        drawing.end
    };

    let filled_shape =
        brush.filled && matches!(drawing.tool, PixelTool::Rectangle | PixelTool::Ellipse);
    let base = match drawing.tool {
        PixelTool::Pencil | PixelTool::Eraser => drawing.path.clone(),
        PixelTool::Line => pixels_on_line(drawing.start, end),
        PixelTool::Rectangle if filled_shape => filled_rectangle(drawing.start, end),
        PixelTool::Rectangle => rectangle_outline(drawing.start, end),
        PixelTool::Ellipse if filled_shape => ellipse_pixels(drawing.start, end, true),
        PixelTool::Ellipse => ellipse_pixels(drawing.start, end, false),
        PixelTool::Fill | PixelTool::Eyedropper | PixelTool::ReplaceColor => Vec::new(),
    };

    let mut pixels = BTreeSet::new();
    if filled_shape {
        for pixel in base {
            insert_with_symmetry(&mut pixels, pixel, width, height, brush);
        }
    } else {
        for center in base {
            for pixel in brush_stamp(center, brush.size, brush.shape, width, height) {
                insert_with_symmetry(&mut pixels, pixel, width, height, brush);
            }
        }
    }
    pixels.into_iter().collect()
}

fn constrained_endpoint(
    start: (u16, u16),
    end: (u16, u16),
    tool: PixelTool,
    width: u16,
    height: u16,
) -> (u16, u16) {
    let delta_x = i32::from(end.0) - i32::from(start.0);
    let delta_y = i32::from(end.1) - i32::from(start.1);
    let absolute_x = delta_x.abs();
    let absolute_y = delta_y.abs();
    let (sign_x, horizontal_limit) = direction_and_limit(start.0, width, delta_x);
    let (sign_y, vertical_limit) = direction_and_limit(start.1, height, delta_y);

    let (x, y) = if tool == PixelTool::Line {
        if absolute_x > absolute_y.saturating_mul(2) {
            (i32::from(end.0), i32::from(start.1))
        } else if absolute_y > absolute_x.saturating_mul(2) {
            (i32::from(start.0), i32::from(end.1))
        } else {
            let distance = absolute_x
                .max(absolute_y)
                .min(horizontal_limit)
                .min(vertical_limit);
            (
                i32::from(start.0) + sign_x * distance,
                i32::from(start.1) + sign_y * distance,
            )
        }
    } else {
        let distance = absolute_x
            .max(absolute_y)
            .min(horizontal_limit)
            .min(vertical_limit);
        (
            i32::from(start.0) + sign_x * distance,
            i32::from(start.1) + sign_y * distance,
        )
    };
    (x as u16, y as u16)
}

fn direction_and_limit(start: u16, length: u16, delta: i32) -> (i32, i32) {
    if delta < 0 {
        (-1, i32::from(start))
    } else if delta > 0 {
        (1, i32::from(length - 1 - start))
    } else {
        let negative = i32::from(start);
        let positive = i32::from(length - 1 - start);
        if positive >= negative {
            (1, positive)
        } else {
            (-1, negative)
        }
    }
}

fn brush_stamp(
    center: (u16, u16),
    size: u16,
    shape: BrushShape,
    width: u16,
    height: u16,
) -> Vec<(u16, u16)> {
    let size = i32::from(size);
    let low = -((size - 1) / 2);
    let high = size / 2;
    let radius = f64::from(size) * 0.5 - 0.25;
    let mut pixels = Vec::with_capacity((size * size) as usize);
    for offset_y in low..=high {
        for offset_x in low..=high {
            if shape == BrushShape::Circle {
                let local_x = f64::from(offset_x) - f64::from(low);
                let local_y = f64::from(offset_y) - f64::from(low);
                let center_offset = (f64::from(size) - 1.0) * 0.5;
                let delta_x = local_x - center_offset;
                let delta_y = local_y - center_offset;
                if delta_x * delta_x + delta_y * delta_y > radius * radius {
                    continue;
                }
            }
            let x = i32::from(center.0) + offset_x;
            let y = i32::from(center.1) + offset_y;
            if x >= 0 && y >= 0 && x < i32::from(width) && y < i32::from(height) {
                pixels.push((x as u16, y as u16));
            }
        }
    }
    pixels
}

fn insert_with_symmetry(
    pixels: &mut BTreeSet<(u16, u16)>,
    pixel: (u16, u16),
    width: u16,
    height: u16,
    brush: Brush,
) {
    let mirrored_x = width - 1 - pixel.0;
    let mirrored_y = height - 1 - pixel.1;
    pixels.insert(pixel);
    if brush.mirror_horizontal {
        pixels.insert((mirrored_x, pixel.1));
    }
    if brush.mirror_vertical {
        pixels.insert((pixel.0, mirrored_y));
    }
    if brush.mirror_horizontal && brush.mirror_vertical {
        pixels.insert((mirrored_x, mirrored_y));
    }
}

fn filled_rectangle(start: (u16, u16), end: (u16, u16)) -> Vec<(u16, u16)> {
    let (left, right) = ordered(start.0, end.0);
    let (top, bottom) = ordered(start.1, end.1);
    let mut pixels =
        Vec::with_capacity(usize::from(right - left + 1) * usize::from(bottom - top + 1));
    for y in top..=bottom {
        for x in left..=right {
            pixels.push((x, y));
        }
    }
    pixels
}

fn rectangle_outline(start: (u16, u16), end: (u16, u16)) -> Vec<(u16, u16)> {
    let (left, right) = ordered(start.0, end.0);
    let (top, bottom) = ordered(start.1, end.1);
    let mut pixels = BTreeSet::new();
    for x in left..=right {
        pixels.insert((x, top));
        pixels.insert((x, bottom));
    }
    for y in top..=bottom {
        pixels.insert((left, y));
        pixels.insert((right, y));
    }
    pixels.into_iter().collect()
}

fn ellipse_pixels(start: (u16, u16), end: (u16, u16), filled: bool) -> Vec<(u16, u16)> {
    let (left, right) = ordered(start.0, end.0);
    let (top, bottom) = ordered(start.1, end.1);
    let mut interior = BTreeSet::new();
    for y in top..=bottom {
        for x in left..=right {
            if ellipse_contains(x, y, left, right, top, bottom) {
                interior.insert((x, y));
            }
        }
    }
    let center_x = (u32::from(left) + u32::from(right)) / 2;
    let center_y = (u32::from(top) + u32::from(bottom)) / 2;
    for x in center_x as u16..=(u32::from(left) + u32::from(right)).div_ceil(2) as u16 {
        interior.insert((x, top));
        interior.insert((x, bottom));
    }
    for y in center_y as u16..=(u32::from(top) + u32::from(bottom)).div_ceil(2) as u16 {
        interior.insert((left, y));
        interior.insert((right, y));
    }
    if filled {
        return interior.into_iter().collect();
    }

    interior
        .iter()
        .copied()
        .filter(|(x, y)| {
            [
                (i32::from(*x) - 1, i32::from(*y)),
                (i32::from(*x) + 1, i32::from(*y)),
                (i32::from(*x), i32::from(*y) - 1),
                (i32::from(*x), i32::from(*y) + 1),
            ]
            .into_iter()
            .any(|(neighbor_x, neighbor_y)| {
                neighbor_x < 0
                    || neighbor_y < 0
                    || !interior.contains(&(neighbor_x as u16, neighbor_y as u16))
            })
        })
        .collect()
}

fn ellipse_contains(x: u16, y: u16, left: u16, right: u16, top: u16, bottom: u16) -> bool {
    if left == right {
        return x == left;
    }
    if top == bottom {
        return y == top;
    }
    let center_x = (f64::from(left) + f64::from(right)) * 0.5;
    let center_y = (f64::from(top) + f64::from(bottom)) * 0.5;
    let radius_x = f64::from(right - left) * 0.5;
    let radius_y = f64::from(bottom - top) * 0.5;
    let normalized_x = (f64::from(x) - center_x) / radius_x;
    let normalized_y = (f64::from(y) - center_y) / radius_y;
    normalized_x * normalized_x + normalized_y * normalized_y <= 1.0 + f64::EPSILON
}

const fn ordered(first: u16, second: u16) -> (u16, u16) {
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}

pub fn pixels_on_line(start: (u16, u16), end: (u16, u16)) -> Vec<(u16, u16)> {
    let (mut x, mut y) = (i32::from(start.0), i32::from(start.1));
    let (end_x, end_y) = (i32::from(end.0), i32::from(end.1));
    let delta_x = (end_x - x).abs();
    let step_x = if x < end_x { 1 } else { -1 };
    let delta_y = -(end_y - y).abs();
    let step_y = if y < end_y { 1 } else { -1 };
    let mut error = delta_x + delta_y;
    let mut pixels = Vec::new();

    loop {
        pixels.push((x as u16, y as u16));
        if x == end_x && y == end_y {
            break;
        }
        let doubled = error * 2;
        if doubled >= delta_y {
            error += delta_y;
            x += step_x;
        }
        if doubled <= delta_x {
            error += delta_x;
            y += step_y;
        }
    }
    pixels
}
