use base64::{engine::general_purpose::STANDARD, Engine as _};
use block::Block;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

pub const DEFAULT_PIXEL_ART_SIZE: u16 = 32;
pub const MAX_PIXEL_ART_SIZE: u16 = 2048;

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
    revision: u64,
}

#[derive(Deserialize)]
struct PixelArtData {
    width: u16,
    height: u16,
    #[serde(deserialize_with = "deserialize_pixels")]
    pixels: Vec<u8>,
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
        Ok(Self {
            width: data.width,
            height: data.height,
            pixels: data.pixels,
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

    const TYPE_ID: Uuid = Uuid::from_u128(0x7069_7865_6c2d_6172_742d_626c_6f63_6b01);

    fn apply_operation(art: &mut Self, operation: &Self::Operation) {
        match operation {
            PixelArtOperation::Paint { pixels } => art.paint(pixels),
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
#[path = "pixel_art/tests/pixel_art_invalid_resize_does_not_change_canvas.rs"]
mod pixel_art_invalid_resize_does_not_change_canvas;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_new_is_32_by_32_and_transparent.rs"]
mod pixel_art_new_is_32_by_32_and_transparent;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_paint_updates_only_valid_targeted_pixels.rs"]
mod pixel_art_paint_updates_only_valid_targeted_pixels;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_resize_preserves_pixels_for_each_anchor.rs"]
mod pixel_art_resize_preserves_pixels_for_each_anchor;
#[cfg(test)]
#[path = "pixel_art/tests/pixel_art_serialization_round_trips_without_data_loss.rs"]
mod pixel_art_serialization_round_trips_without_data_loss;
