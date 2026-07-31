use base64::{engine::general_purpose::STANDARD, Engine as _};
use block::{Block, BlockHistory, HistoryDirection};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

pub const DEFAULT_PIXEL_ART_SIZE: u16 = 32;
pub const MAX_PIXEL_ART_SIZE: u16 = 2048;
pub const MAX_PIXEL_ART_PALETTE_COLORS: usize = 32;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct PixelColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl PixelColor {
    pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);

    pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn rgba(self) -> [u8; 4] {
        [self.red, self.green, self.blue, self.alpha]
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct PixelUpdate {
    pub x: u16,
    pub y: u16,
    pub color: PixelColor,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelArtAnchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    #[default]
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum PixelArtOperation {
    Paint {
        pixels: Vec<PixelUpdate>,
    },
    Fill {
        x: u16,
        y: u16,
        color: PixelColor,
    },
    ReplaceColor {
        from: PixelColor,
        to: PixelColor,
    },
    SetPalette {
        colors: Vec<PixelColor>,
    },
    Clear,
    Resize {
        width: u16,
        height: u16,
        anchor: PixelArtAnchor,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PixelArt {
    width: u16,
    height: u16,
    #[serde(
        serialize_with = "serialize_pixels",
        deserialize_with = "deserialize_pixels"
    )]
    pixels: Vec<u8>,
    palette: Vec<PixelColor>,
    revision: u64,
}

pub struct PixelArtHistory;

pub struct PixelArtHistoryAction {
    kind: PixelArtHistoryActionKind,
}

enum PixelArtHistoryActionKind {
    Pixels(Vec<PixelDelta>),
    Palette {
        before: Vec<PixelColor>,
        after: Vec<PixelColor>,
    },
    Resize {
        before: PixelArt,
        after: PixelArt,
        anchor: PixelArtAnchor,
    },
}

struct PixelDelta {
    x: u16,
    y: u16,
    before: PixelColor,
    after: PixelColor,
}

#[derive(Deserialize)]
struct PixelArtData {
    width: u16,
    height: u16,
    #[serde(deserialize_with = "deserialize_pixels")]
    pixels: Vec<u8>,
    palette: Vec<PixelColor>,
    revision: u64,
}

impl<'de> Deserialize<'de> for PixelArt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = PixelArtData::deserialize(deserializer)?;
        if !valid_dimension(data.width) || !valid_dimension(data.height) {
            return Err(D::Error::custom("pixel art dimensions are out of range"));
        }
        if data.pixels.len() != pixel_bytes(data.width, data.height) {
            return Err(D::Error::custom(
                "pixel art buffer length does not match its dimensions",
            ));
        }
        if data.palette.len() > MAX_PIXEL_ART_PALETTE_COLORS
            || data
                .palette
                .iter()
                .enumerate()
                .any(|(index, color)| data.palette[..index].contains(color))
        {
            return Err(D::Error::custom("pixel art palette is invalid"));
        }
        Ok(Self {
            width: data.width,
            height: data.height,
            pixels: data.pixels,
            palette: data.palette,
            revision: data.revision,
        })
    }
}

impl PixelArt {
    pub fn new() -> Self {
        Self::with_size(DEFAULT_PIXEL_ART_SIZE, DEFAULT_PIXEL_ART_SIZE)
    }

    fn with_size(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; pixel_bytes(width, height)],
            palette: default_palette(),
            revision: 0,
        }
    }

    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn height(&self) -> u16 {
        self.height
    }

    pub fn rgba_bytes(&self) -> &[u8] {
        &self.pixels
    }

    pub fn palette(&self) -> &[PixelColor] {
        &self.palette
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn pixel(&self, x: u16, y: u16) -> Option<PixelColor> {
        let offset = self.pixel_offset(x, y)?;
        Some(PixelColor::new(
            self.pixels[offset],
            self.pixels[offset + 1],
            self.pixels[offset + 2],
            self.pixels[offset + 3],
        ))
    }

    fn pixel_offset(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some((usize::from(y) * usize::from(self.width) + usize::from(x)) * 4)
    }

    fn paint(&mut self, updates: &[PixelUpdate]) {
        let mut changed = false;
        for update in updates {
            let Some(offset) = self.pixel_offset(update.x, update.y) else {
                continue;
            };
            let color = update.color.rgba();
            if self.pixels[offset..offset + 4] != color {
                self.pixels[offset..offset + 4].copy_from_slice(&color);
                changed = true;
            }
        }
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn fill(&mut self, x: u16, y: u16, color: PixelColor) {
        let Some(offset) = self.pixel_offset(x, y) else {
            return;
        };
        let replacement = color.rgba();
        let target: [u8; 4] = self.pixels[offset..offset + 4]
            .try_into()
            .expect("pixel colors always contain four channels");
        if target == replacement {
            return;
        }

        let width = usize::from(self.width);
        let height = usize::from(self.height);
        let start = usize::from(y) * width + usize::from(x);
        let mut pending = vec![start];
        self.pixels[offset..offset + 4].copy_from_slice(&replacement);

        while let Some(index) = pending.pop() {
            let pixel_x = index % width;
            let pixel_y = index / width;
            let neighbors = [
                (pixel_x > 0).then_some(index.wrapping_sub(1)),
                (pixel_x + 1 < width).then_some(index + 1),
                (pixel_y > 0).then_some(index.wrapping_sub(width)),
                (pixel_y + 1 < height).then_some(index + width),
            ];

            for neighbor in neighbors.into_iter().flatten() {
                let neighbor_offset = neighbor * 4;
                if self.pixels[neighbor_offset..neighbor_offset + 4] == target {
                    self.pixels[neighbor_offset..neighbor_offset + 4].copy_from_slice(&replacement);
                    pending.push(neighbor);
                }
            }
        }

        self.revision = self.revision.wrapping_add(1);
    }

    fn replace_color(&mut self, from: PixelColor, to: PixelColor) {
        if from == to {
            return;
        }
        let from = from.rgba();
        let to = to.rgba();
        let mut changed = false;
        for pixel in self.pixels.chunks_exact_mut(4) {
            if pixel == from {
                pixel.copy_from_slice(&to);
                changed = true;
            }
        }
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn set_palette(&mut self, colors: &[PixelColor]) {
        let colors = normalized_palette(colors);
        if self.palette != colors {
            self.palette = colors;
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn clear(&mut self) {
        if self.pixels.iter().any(|channel| *channel != 0) {
            self.pixels.fill(0);
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn resize(&mut self, width: u16, height: u16, anchor: PixelArtAnchor) {
        if !valid_dimension(width)
            || !valid_dimension(height)
            || (width == self.width && height == self.height)
        {
            return;
        }

        let copy_width = self.width.min(width);
        let copy_height = self.height.min(height);
        let (source_x, destination_x) =
            aligned_offsets(self.width, width, copy_width, horizontal_alignment(anchor));
        let (source_y, destination_y) =
            aligned_offsets(self.height, height, copy_height, vertical_alignment(anchor));
        let mut resized = vec![0; pixel_bytes(width, height)];

        for row in 0..copy_height {
            let source = ((usize::from(source_y + row) * usize::from(self.width))
                + usize::from(source_x))
                * 4;
            let destination = ((usize::from(destination_y + row) * usize::from(width))
                + usize::from(destination_x))
                * 4;
            let length = usize::from(copy_width) * 4;
            resized[destination..destination + length]
                .copy_from_slice(&self.pixels[source..source + length]);
        }

        self.width = width;
        self.height = height;
        self.pixels = resized;
        self.revision = self.revision.wrapping_add(1);
    }
}

impl Default for PixelArt {
    fn default() -> Self {
        Self::new()
    }
}

impl Block for PixelArt {
    type Operation = PixelArtOperation;
    type History = PixelArtHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7069_7865_6c2d_6172_742d_626c_6f63_6b01);

    fn apply_operation(art: &mut Self, operation: &Self::Operation) {
        match operation {
            PixelArtOperation::Paint { pixels } => art.paint(pixels),
            PixelArtOperation::Fill { x, y, color } => art.fill(*x, *y, *color),
            PixelArtOperation::ReplaceColor { from, to } => art.replace_color(*from, *to),
            PixelArtOperation::SetPalette { colors } => art.set_palette(colors),
            PixelArtOperation::Clear => art.clear(),
            PixelArtOperation::Resize {
                width,
                height,
                anchor,
            } => art.resize(*width, *height, *anchor),
        }
    }

    fn implicit_name(&self) -> String {
        "Pixel Art".into()
    }
}

impl BlockHistory<PixelArt> for PixelArtHistory {
    type Action = PixelArtHistoryAction;

    fn action(
        before: &PixelArt,
        after: &PixelArt,
        operations: &[PixelArtOperation],
    ) -> Option<Self::Action> {
        if before == after {
            return None;
        }
        let kind = if before.width != after.width || before.height != after.height {
            let anchor = operations
                .iter()
                .rev()
                .find_map(|operation| match operation {
                    PixelArtOperation::Resize { anchor, .. } => Some(*anchor),
                    _ => None,
                })?;
            PixelArtHistoryActionKind::Resize {
                before: before.clone(),
                after: after.clone(),
                anchor,
            }
        } else if before.palette != after.palette {
            PixelArtHistoryActionKind::Palette {
                before: before.palette.clone(),
                after: after.palette.clone(),
            }
        } else {
            let deltas = pixel_deltas(before, after);
            if deltas.is_empty() {
                return None;
            }
            PixelArtHistoryActionKind::Pixels(deltas)
        };
        Some(PixelArtHistoryAction { kind })
    }

    fn action_bytes(action: &Self::Action) -> usize {
        match &action.kind {
            PixelArtHistoryActionKind::Pixels(deltas) => deltas.len() * size_of::<PixelDelta>(),
            PixelArtHistoryActionKind::Palette { before, after } => {
                (before.len() + after.len()) * size_of::<PixelColor>()
            }
            PixelArtHistoryActionKind::Resize { before, after, .. } => {
                before.pixels.len() + after.pixels.len()
            }
        }
    }

    fn operations(
        current: &PixelArt,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) -> Vec<PixelArtOperation> {
        let to_after = direction == HistoryDirection::Redo;
        match &action.kind {
            PixelArtHistoryActionKind::Pixels(deltas) => {
                let pixels = deltas
                    .iter()
                    .filter_map(|delta| {
                        let (expected, desired) = if to_after {
                            (delta.before, delta.after)
                        } else {
                            (delta.after, delta.before)
                        };
                        (current.pixel(delta.x, delta.y) == Some(expected)).then_some(PixelUpdate {
                            x: delta.x,
                            y: delta.y,
                            color: desired,
                        })
                    })
                    .collect::<Vec<_>>();
                (!pixels.is_empty())
                    .then_some(PixelArtOperation::Paint { pixels })
                    .into_iter()
                    .collect()
            }
            PixelArtHistoryActionKind::Palette { before, after } => {
                let (expected, desired) = if to_after {
                    (before, after)
                } else {
                    (after, before)
                };
                (current.palette == *expected)
                    .then(|| PixelArtOperation::SetPalette {
                        colors: desired.clone(),
                    })
                    .into_iter()
                    .collect()
            }
            PixelArtHistoryActionKind::Resize {
                before,
                after,
                anchor,
            } => {
                let (source, desired) = if to_after {
                    (before, after)
                } else {
                    (after, before)
                };
                let resize = PixelArtOperation::Resize {
                    width: desired.width,
                    height: desired.height,
                    anchor: *anchor,
                };
                let mut expected = source.clone();
                PixelArt::apply_operation(&mut expected, &resize);
                let mut resized_current = current.clone();
                PixelArt::apply_operation(&mut resized_current, &resize);
                let pixels = pixel_deltas(&expected, desired)
                    .into_iter()
                    .filter(|delta| resized_current.pixel(delta.x, delta.y) == Some(delta.before))
                    .map(|delta| PixelUpdate {
                        x: delta.x,
                        y: delta.y,
                        color: delta.after,
                    })
                    .collect::<Vec<_>>();
                let mut operations = vec![resize];
                if !pixels.is_empty() {
                    operations.push(PixelArtOperation::Paint { pixels });
                }
                operations
            }
        }
    }
}

fn pixel_deltas(before: &PixelArt, after: &PixelArt) -> Vec<PixelDelta> {
    if before.width != after.width || before.height != after.height {
        return Vec::new();
    }
    let mut deltas = Vec::new();
    for y in 0..before.height {
        for x in 0..before.width {
            let before = before.pixel(x, y).unwrap();
            let after = after.pixel(x, y).unwrap();
            if before != after {
                deltas.push(PixelDelta {
                    x,
                    y,
                    before,
                    after,
                });
            }
        }
    }
    deltas
}

fn default_palette() -> Vec<PixelColor> {
    [
        PixelColor::TRANSPARENT,
        PixelColor::new(0, 0, 0, 255),
        PixelColor::new(255, 255, 255, 255),
        PixelColor::new(128, 128, 128, 255),
        PixelColor::new(192, 192, 192, 255),
        PixelColor::new(136, 57, 50, 255),
        PixelColor::new(237, 118, 20, 255),
        PixelColor::new(255, 215, 0, 255),
        PixelColor::new(51, 160, 44, 255),
        PixelColor::new(40, 200, 170, 255),
        PixelColor::new(40, 110, 220, 255),
        PixelColor::new(95, 60, 190, 255),
        PixelColor::new(190, 60, 190, 255),
        PixelColor::new(230, 90, 120, 255),
        PixelColor::new(115, 75, 45, 255),
        PixelColor::new(242, 194, 156, 255),
    ]
    .to_vec()
}

fn normalized_palette(colors: &[PixelColor]) -> Vec<PixelColor> {
    let mut palette = Vec::with_capacity(colors.len().min(MAX_PIXEL_ART_PALETTE_COLORS));
    for &color in colors {
        if palette.len() == MAX_PIXEL_ART_PALETTE_COLORS {
            break;
        }
        if !palette.contains(&color) {
            palette.push(color);
        }
    }
    palette
}

#[derive(Clone, Copy)]
enum Alignment {
    Start,
    Center,
    End,
}

fn horizontal_alignment(anchor: PixelArtAnchor) -> Alignment {
    match anchor {
        PixelArtAnchor::TopLeft | PixelArtAnchor::Left | PixelArtAnchor::BottomLeft => {
            Alignment::Start
        }
        PixelArtAnchor::Top | PixelArtAnchor::Center | PixelArtAnchor::Bottom => Alignment::Center,
        PixelArtAnchor::TopRight | PixelArtAnchor::Right | PixelArtAnchor::BottomRight => {
            Alignment::End
        }
    }
}

fn vertical_alignment(anchor: PixelArtAnchor) -> Alignment {
    match anchor {
        PixelArtAnchor::TopLeft | PixelArtAnchor::Top | PixelArtAnchor::TopRight => {
            Alignment::Start
        }
        PixelArtAnchor::Left | PixelArtAnchor::Center | PixelArtAnchor::Right => Alignment::Center,
        PixelArtAnchor::BottomLeft | PixelArtAnchor::Bottom | PixelArtAnchor::BottomRight => {
            Alignment::End
        }
    }
}

fn aligned_offsets(old: u16, new: u16, overlap: u16, alignment: Alignment) -> (u16, u16) {
    match alignment {
        Alignment::Start => (0, 0),
        Alignment::Center => ((old - overlap) / 2, (new - overlap) / 2),
        Alignment::End => (old - overlap, new - overlap),
    }
}

const fn valid_dimension(value: u16) -> bool {
    value >= 1 && value <= MAX_PIXEL_ART_SIZE
}

fn pixel_bytes(width: u16, height: u16) -> usize {
    usize::from(width) * usize::from(height) * 4
}

fn serialize_pixels<S>(pixels: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&STANDARD.encode(pixels))
}

fn deserialize_pixels<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let encoded = String::deserialize(deserializer)?;
    STANDARD.decode(encoded).map_err(D::Error::custom)
}

#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_clear_restores_all_pixels_to_transparency.rs"]
mod pixel_art_clear_restores_all_pixels_to_transparency;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_fill_ignores_invalid_and_unchanged_targets.rs"]
mod pixel_art_fill_ignores_invalid_and_unchanged_targets;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_fill_recolors_only_connected_pixels.rs"]
mod pixel_art_fill_recolors_only_connected_pixels;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_history_undoes_and_redoes_paint.rs"]
mod pixel_art_history_undoes_and_redoes_paint;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_history_undoes_and_redoes_palette.rs"]
mod pixel_art_history_undoes_and_redoes_palette;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_history_undoes_and_redoes_replace_color.rs"]
mod pixel_art_history_undoes_and_redoes_replace_color;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_invalid_resize_does_not_change_canvas.rs"]
mod pixel_art_invalid_resize_does_not_change_canvas;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_new_has_default_palette.rs"]
mod pixel_art_new_has_default_palette;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_new_is_32_by_32_and_transparent.rs"]
mod pixel_art_new_is_32_by_32_and_transparent;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_paint_updates_only_valid_targeted_pixels.rs"]
mod pixel_art_paint_updates_only_valid_targeted_pixels;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_replace_color_ignores_no_op_and_missing_source.rs"]
mod pixel_art_replace_color_ignores_no_op_and_missing_source;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_replace_color_replaces_every_exact_match.rs"]
mod pixel_art_replace_color_replaces_every_exact_match;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_resize_preserves_pixels_for_each_anchor.rs"]
mod pixel_art_resize_preserves_pixels_for_each_anchor;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_serialization_rejects_invalid_palette.rs"]
mod pixel_art_serialization_rejects_invalid_palette;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_serialization_round_trips_without_data_loss.rs"]
mod pixel_art_serialization_round_trips_without_data_loss;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_set_palette_deduplicates_and_limits_colors.rs"]
mod pixel_art_set_palette_deduplicates_and_limits_colors;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_deltas_capture_only_changed_pixels.rs"]
mod pixel_deltas_capture_only_changed_pixels;
