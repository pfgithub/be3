use std::collections::{BTreeMap, HashMap};

use egui::epaint::Primitive as EguiPrimitive;
use egui::{ImageData, TextureId, TexturesDelta};
use sha2::{Digest as _, Sha256};

use crate::format::{Content, Frame, Primitive, Snapshot, Texture, TextureKey, Triangle, Vertex};

const USER_TEXTURE: u64 = 1 << 63;

type SourceId = u64;

#[derive(Default)]
pub struct TextureStore {
    images: HashMap<SourceId, Image>,
}

struct Image {
    size: [u32; 2],
    pixels: Vec<[u8; 4]>,
}

impl Image {
    fn crop(&self, rect: [u32; 4]) -> Vec<[u8; 4]> {
        let [left, top, right, bottom] = rect;
        let mut pixels = Vec::with_capacity(((right - left) * (bottom - top)) as usize);
        for row in top..bottom {
            for column in left..right {
                let index = (row * self.size[0] + column) as usize;
                pixels.push(self.pixels.get(index).copied().unwrap_or([0; 4]));
            }
        }
        pixels
    }
}

impl TextureStore {
    pub fn apply(&mut self, delta: &TexturesDelta) {
        for (id, patch) in &delta.set {
            let ImageData::Color(image) = &patch.image;
            let pixels: Vec<[u8; 4]> = image.pixels.iter().map(|color| color.to_array()).collect();
            let size = [image.size[0] as u32, image.size[1] as u32];
            let entry = self.images.entry(key(*id));
            match patch.pos {
                None => {
                    entry
                        .and_modify(|target| {
                            target.size = size;
                            target.pixels.clone_from(&pixels);
                        })
                        .or_insert_with(|| Image { size, pixels });
                }
                Some([x, y]) => {
                    let Some(target) = self.images.get_mut(&key(*id)) else {
                        continue;
                    };
                    for row in 0..image.size[1] {
                        for column in 0..image.size[0] {
                            let target_index = (y + row) * target.size[0] as usize + (x + column);
                            if let Some(texel) = target.pixels.get_mut(target_index) {
                                *texel = pixels[row * image.size[0] + column];
                            }
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.images.remove(&key(*id));
        }
    }

    fn image(&self, source: SourceId) -> Result<&Image, String> {
        self.images
            .get(&source)
            .ok_or_else(|| "the painting uses a texture that was never uploaded".to_owned())
    }
}

fn key(id: TextureId) -> SourceId {
    match id {
        TextureId::Managed(id) => id,
        TextureId::User(id) => id | USER_TEXTURE,
    }
}

struct Crops<'a> {
    store: &'a TextureStore,
    keys: HashMap<(SourceId, [u32; 4]), TextureKey>,
    textures: BTreeMap<TextureKey, Texture>,
}

impl Crops<'_> {
    fn triangles(&mut self, mesh: &egui::Mesh) -> Result<Vec<Triangle>, String> {
        let source = key(mesh.texture_id);
        let size = self.store.image(source)?.size;
        let mut triangles = Vec::with_capacity(mesh.indices.len() / 3);
        for indices in mesh.indices.as_chunks::<3>().0 {
            let corners = [0, 1, 2].map(|corner| mesh.vertices[indices[corner] as usize]);
            let uv = corners.map(|corner| [corner.uv.x, corner.uv.y]);
            let rect = footprint(uv, size);
            triangles.push(Triangle {
                texture: self.texture(source, rect)?,
                corners: [0, 1, 2].map(|corner| Vertex {
                    pos: [corners[corner].pos.x, corners[corner].pos.y],
                    uv: within(uv[corner], rect, size),
                    color: corners[corner].color.to_array(),
                }),
            });
        }
        Ok(triangles)
    }

    fn texture(&mut self, source: SourceId, rect: [u32; 4]) -> Result<TextureKey, String> {
        if let Some(key) = self.keys.get(&(source, rect)) {
            return Ok(*key);
        }
        let image = self.store.image(source)?;
        let pixels = image.crop(rect);
        let size = [rect[2] - rect[0], rect[3] - rect[1]];
        let key = fingerprint(size, &pixels);
        self.keys.insert((source, rect), key);
        if let std::collections::btree_map::Entry::Vacant(entry) = self.textures.entry(key) {
            entry.insert(Texture::encode(size, &pixels)?);
        }
        Ok(key)
    }
}

fn footprint(uv: [[f32; 2]; 3], size: [u32; 2]) -> [u32; 4] {
    let axis = |axis: usize, extent: u32| {
        let extent = extent as f32;
        let coordinates = uv.map(|corner| corner[axis] * extent);
        let low = coordinates.iter().fold(f32::MAX, |low, at| low.min(*at));
        let high = coordinates.iter().fold(f32::MIN, |high, at| high.max(*at));
        let low = low.floor().clamp(0.0, (extent - 1.0).max(0.0)) as u32;
        let high = (high.ceil().clamp(0.0, extent) as u32).max(low + 1);
        (low, high)
    };
    let (left, right) = axis(0, size[0]);
    let (top, bottom) = axis(1, size[1]);
    [left, top, right, bottom]
}

fn within(uv: [f32; 2], rect: [u32; 4], size: [u32; 2]) -> [f32; 2] {
    let at = |axis: usize| {
        let texel = uv[axis] * size[axis] as f32 - rect[axis] as f32;
        texel / (rect[axis + 2] - rect[axis]) as f32
    };
    [at(0), at(1)]
}

fn fingerprint(size: [u32; 2], pixels: &[[u8; 4]]) -> TextureKey {
    let mut hash = Sha256::new();
    hash.update(size[0].to_le_bytes());
    hash.update(size[1].to_le_bytes());
    hash.update(pixels.as_flattened());
    u64::from_le_bytes(hash.finalize()[..8].try_into().unwrap())
}

pub fn capture(
    context: &egui::Context,
    output: &egui::FullOutput,
    textures: &TextureStore,
) -> Result<Snapshot, String> {
    let pixels_per_point = output.pixels_per_point;
    let screen = context.content_rect();
    let tessellated = context.tessellate(output.shapes.clone(), pixels_per_point);

    let mut crops = Crops {
        store: textures,
        keys: HashMap::new(),
        textures: BTreeMap::new(),
    };
    let mut primitives = Vec::with_capacity(tessellated.len());
    for clipped in tessellated {
        let clip = rectangle(clipped.clip_rect);
        let content = match clipped.primitive {
            EguiPrimitive::Mesh(mesh) => Content::Mesh(crops.triangles(&mesh)?),
            EguiPrimitive::Callback(callback) => Content::Callback(rectangle(callback.rect)),
        };
        primitives.push(Primitive { clip, content });
    }

    Ok(Snapshot::of(
        Frame {
            size: [
                (screen.width() * pixels_per_point).round() as u32,
                (screen.height() * pixels_per_point).round() as u32,
            ],
            pixels_per_point,
            background: context.global_style().visuals.panel_fill.to_array(),
            primitives,
        },
        crops.textures,
    ))
}

fn rectangle(rect: egui::Rect) -> [f32; 4] {
    [rect.min.x, rect.min.y, rect.max.x, rect.max.y]
}
