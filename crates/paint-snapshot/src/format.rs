use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};

use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 8] = b"BE3PAINT";
const VERSION: u32 = 1;

pub type TextureKey = u64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub size: [u32; 2],
    pub pixels_per_point: f32,
    pub background: [u8; 4],
    pub primitives: Vec<Primitive>,
    pub textures: BTreeMap<TextureKey, Texture>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Primitive {
    pub clip: [f32; 4],
    pub content: Content,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Content {
    Mesh(Mesh),
    Callback([f32; 4]),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    pub texture: TextureKey,
    pub indices: Vec<u32>,
    pub vertices: Vec<Vertex>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Texture {
    pub size: [u32; 2],
    pub alpha: bool,
    pub png: Vec<u8>,
}

impl Texture {
    pub fn pixels(&self) -> Result<Vec<[u8; 4]>, String> {
        let image = image::load_from_memory_with_format(&self.png, image::ImageFormat::Png)
            .map_err(|error| format!("texture is not a readable png: {error}"))?;
        if self.alpha {
            return Ok(image
                .to_luma8()
                .pixels()
                .map(|pixel| [pixel.0[0]; 4])
                .collect());
        }
        Ok(image.to_rgba8().pixels().map(|pixel| pixel.0).collect())
    }

    pub fn encode(size: [u32; 2], pixels: &[[u8; 4]]) -> Result<Self, String> {
        let alpha = pixels
            .iter()
            .all(|pixel| pixel.iter().all(|channel| *channel == pixel[3]));
        let mut png = Vec::new();
        let target = &mut std::io::Cursor::new(&mut png);
        if alpha {
            let flat = pixels.iter().map(|pixel| pixel[3]).collect();
            image::GrayImage::from_vec(size[0], size[1], flat)
                .ok_or("texture pixels do not match its size")?
                .write_to(target, image::ImageFormat::Png)
        } else {
            let flat = pixels.iter().flatten().copied().collect();
            image::RgbaImage::from_vec(size[0], size[1], flat)
                .ok_or("texture pixels do not match its size")?
                .write_to(target, image::ImageFormat::Png)
        }
        .map_err(|error| format!("could not encode texture: {error}"))?;
        Ok(Self { size, alpha, png })
    }
}

impl Snapshot {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let body = bincode::serialize(self)
            .map_err(|error| format!("could not serialize snapshot: {error}"))?;
        let mut bytes = Vec::with_capacity(body.len() / 2);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        let mut encoder = DeflateEncoder::new(bytes, Compression::best());
        encoder
            .write_all(&body)
            .map_err(|error| format!("could not compress snapshot: {error}"))?;
        encoder
            .finish()
            .map_err(|error| format!("could not compress snapshot: {error}"))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let header = bytes.len().min(MAGIC.len() + 4);
        if header < MAGIC.len() + 4 || &bytes[..MAGIC.len()] != MAGIC {
            return Err("not a paint snapshot".into());
        }
        let version = u32::from_le_bytes(bytes[MAGIC.len()..header].try_into().unwrap());
        if version != VERSION {
            return Err(format!(
                "snapshot is version {version}, this tool reads version {VERSION}"
            ));
        }
        let mut body = Vec::new();
        DeflateDecoder::new(&bytes[header..])
            .read_to_end(&mut body)
            .map_err(|error| format!("could not decompress snapshot: {error}"))?;
        bincode::deserialize(&body)
            .map_err(|error| format!("could not deserialize snapshot: {error}"))
    }
}
