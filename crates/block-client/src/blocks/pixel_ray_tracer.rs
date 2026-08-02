use base64::{engine::general_purpose::STANDARD, Engine as _};
use block::Block;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

pub const PIXEL_RAY_TRACER_SIZE: u16 = 128;
pub const PIXEL_RAY_TRACER_BACKGROUND: u8 = 7;
pub const PIXEL_RAY_TRACER_PALETTE: [[u8; 3]; 16] = [
    [0x00, 0x00, 0x00],
    [0x1d, 0x2b, 0x53],
    [0x7e, 0x25, 0x53],
    [0x00, 0x87, 0x51],
    [0xab, 0x52, 0x36],
    [0x5f, 0x57, 0x4f],
    [0xc2, 0xc3, 0xc7],
    [0xff, 0xf1, 0xe8],
    [0xff, 0x00, 0x4d],
    [0xff, 0xa3, 0x00],
    [0xff, 0xec, 0x27],
    [0x00, 0xe4, 0x36],
    [0x29, 0xad, 0xff],
    [0x83, 0x76, 0x9c],
    [0xff, 0x77, 0xa8],
    [0xff, 0xcc, 0xaa],
];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RayEntity {
    Surface {
        id: u64,
        start: Point,
        end: Point,
        color_index: u8,
        roughness: f32,
        metalness: f32,
        transmission: f32,
        refractive_index: f32,
    },
    Water {
        id: u64,
        start: Point,
        end: Point,
    },
    Light {
        id: u64,
        position: Point,
        color_index: u8,
        intensity: f32,
    },
}

impl RayEntity {
    pub const fn id(&self) -> u64 {
        match self {
            Self::Surface { id, .. } | Self::Water { id, .. } | Self::Light { id, .. } => *id,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct RaySettings {
    pub ray_count: u16,
    pub step_distance: f32,
    pub maximum_steps: u16,
}

impl Default for RaySettings {
    fn default() -> Self {
        Self {
            ray_count: 800,
            step_distance: 0.5,
            maximum_steps: 512,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct PixelUpdate {
    pub x: u16,
    pub y: u16,
    pub color_index: u8,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum PixelRayTracerOperation {
    Paint { pixels: Vec<PixelUpdate> },
    AddEntity { entity: RayEntity },
    UpdateEntity { entity: RayEntity },
    DeleteEntity { id: u64 },
    SetViewRaySettings { settings: RaySettings },
    SetLightingRaySettings { settings: RaySettings },
    Reset,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PixelRayTracer {
    #[serde(
        serialize_with = "serialize_pixels",
        deserialize_with = "deserialize_pixels"
    )]
    pixels: Vec<u8>,
    entities: Vec<RayEntity>,
    view_ray_settings: RaySettings,
    lighting_ray_settings: RaySettings,
    revision: u64,
    #[serde(default)]
    lighting_revision: u64,
}

#[derive(Deserialize)]
struct PixelRayTracerData {
    #[serde(deserialize_with = "deserialize_pixels")]
    pixels: Vec<u8>,
    entities: Vec<RayEntity>,
    view_ray_settings: RaySettings,
    lighting_ray_settings: RaySettings,
    revision: u64,
    #[serde(default)]
    lighting_revision: u64,
}

impl<'de> Deserialize<'de> for PixelRayTracer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let data = PixelRayTracerData::deserialize(deserializer)?;
        let expected = usize::from(PIXEL_RAY_TRACER_SIZE).pow(2);
        if data.pixels.len() != expected
            || data
                .pixels
                .iter()
                .any(|color| usize::from(*color) >= PIXEL_RAY_TRACER_PALETTE.len())
        {
            return Err(D::Error::custom("pixel ray tracer pixel data is invalid"));
        }
        if !valid_settings(data.view_ray_settings)
            || !valid_settings(data.lighting_ray_settings)
            || data.entities.iter().enumerate().any(|(index, entity)| {
                !valid_entity(entity)
                    || data.entities[..index]
                        .iter()
                        .any(|other| other.id() == entity.id())
            })
        {
            return Err(D::Error::custom("pixel ray tracer scene is invalid"));
        }
        Ok(Self {
            pixels: data.pixels,
            entities: data.entities,
            view_ray_settings: data.view_ray_settings,
            lighting_ray_settings: data.lighting_ray_settings,
            revision: data.revision,
            lighting_revision: data.lighting_revision,
        })
    }
}

impl PixelRayTracer {
    pub fn new() -> Self {
        Self {
            pixels: vec![PIXEL_RAY_TRACER_BACKGROUND; usize::from(PIXEL_RAY_TRACER_SIZE).pow(2)],
            entities: Vec::new(),
            view_ray_settings: RaySettings::default(),
            lighting_ray_settings: RaySettings::default(),
            revision: 0,
            lighting_revision: 0,
        }
    }
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
    pub fn entities(&self) -> &[RayEntity] {
        &self.entities
    }
    pub const fn view_ray_settings(&self) -> RaySettings {
        self.view_ray_settings
    }
    pub const fn lighting_ray_settings(&self) -> RaySettings {
        self.lighting_ray_settings
    }
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    pub const fn lighting_revision(&self) -> u64 {
        self.lighting_revision
    }
    pub fn next_entity_id(&self) -> u64 {
        self.entities.iter().map(RayEntity::id).max().unwrap_or(0) + 1
    }

    fn apply(&mut self, operation: &PixelRayTracerOperation) {
        let mut changed = false;
        let mut lighting_changed = false;
        match operation {
            PixelRayTracerOperation::Paint { pixels } => {
                for update in pixels {
                    if update.x < PIXEL_RAY_TRACER_SIZE
                        && update.y < PIXEL_RAY_TRACER_SIZE
                        && usize::from(update.color_index) < PIXEL_RAY_TRACER_PALETTE.len()
                    {
                        let index = usize::from(update.y) * usize::from(PIXEL_RAY_TRACER_SIZE)
                            + usize::from(update.x);
                        if self.pixels[index] != update.color_index {
                            self.pixels[index] = update.color_index;
                            changed = true;
                            lighting_changed = true;
                        }
                    }
                }
            }
            PixelRayTracerOperation::AddEntity { entity } => {
                if valid_entity(entity)
                    && !self.entities.iter().any(|item| item.id() == entity.id())
                {
                    self.entities.push(entity.clone());
                    changed = true;
                    lighting_changed = true;
                }
            }
            PixelRayTracerOperation::UpdateEntity { entity } => {
                if valid_entity(entity) {
                    if let Some(current) = self
                        .entities
                        .iter_mut()
                        .find(|item| item.id() == entity.id())
                    {
                        if current != entity {
                            current.clone_from(entity);
                            changed = true;
                            lighting_changed = true;
                        }
                    }
                }
            }
            PixelRayTracerOperation::DeleteEntity { id } => {
                let old = self.entities.len();
                self.entities.retain(|entity| entity.id() != *id);
                changed = old != self.entities.len();
                lighting_changed = changed;
            }
            PixelRayTracerOperation::SetViewRaySettings { settings } => {
                if valid_settings(*settings) && self.view_ray_settings != *settings {
                    self.view_ray_settings = *settings;
                    changed = true;
                }
            }
            PixelRayTracerOperation::SetLightingRaySettings { settings } => {
                if valid_settings(*settings) && self.lighting_ray_settings != *settings {
                    self.lighting_ray_settings = *settings;
                    changed = true;
                    lighting_changed = true;
                }
            }
            PixelRayTracerOperation::Reset => {
                self.pixels.fill(PIXEL_RAY_TRACER_BACKGROUND);
                self.entities.clear();
                changed = true;
                lighting_changed = true;
            }
        }
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
        if lighting_changed {
            self.lighting_revision = self.lighting_revision.wrapping_add(1);
        }
    }
}

impl Default for PixelRayTracer {
    fn default() -> Self {
        Self::new()
    }
}

impl Block for PixelRayTracer {
    type Operation = PixelRayTracerOperation;
    type History = block::NoHistory;
    const TYPE_ID: Uuid = Uuid::from_u128(0x7069_7865_6c2d_7261_7974_7261_6365_7201);
    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        block.apply(operation);
    }
    fn implicit_name(&self) -> String {
        "Pixel Ray Tracer".into()
    }
}

fn valid_settings(settings: RaySettings) -> bool {
    (1..=2048).contains(&settings.ray_count)
        && (1..=2048).contains(&settings.maximum_steps)
        && settings.step_distance.is_finite()
        && (0.05..=128.0).contains(&settings.step_distance)
}

fn valid_point(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn valid_entity(entity: &RayEntity) -> bool {
    match entity {
        RayEntity::Surface {
            start,
            end,
            color_index,
            roughness,
            metalness,
            transmission,
            refractive_index,
            ..
        } => {
            valid_point(*start)
                && valid_point(*end)
                && usize::from(*color_index) < PIXEL_RAY_TRACER_PALETTE.len()
                && roughness.is_finite()
                && (0.0..=1.0).contains(roughness)
                && metalness.is_finite()
                && (0.0..=1.0).contains(metalness)
                && transmission.is_finite()
                && (0.0..=1.0).contains(transmission)
                && refractive_index.is_finite()
                && (1.0..=3.0).contains(refractive_index)
        }
        RayEntity::Water { start, end, .. } => valid_point(*start) && valid_point(*end),
        RayEntity::Light {
            position,
            color_index,
            intensity,
            ..
        } => {
            valid_point(*position)
                && usize::from(*color_index) < PIXEL_RAY_TRACER_PALETTE.len()
                && intensity.is_finite()
                && (0.1..=8.0).contains(intensity)
        }
    }
}

fn serialize_pixels<S: Serializer>(pixels: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&STANDARD.encode(pixels))
}
fn deserialize_pixels<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    let encoded = String::deserialize(deserializer)?;
    STANDARD.decode(encoded).map_err(D::Error::custom)
}

#[cfg(test)]
mod tests;
