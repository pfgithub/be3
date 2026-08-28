use std::collections::{BTreeMap, HashMap};

use egui::epaint::Primitive as EguiPrimitive;
use egui::{ImageData, TextureId, TexturesDelta};

use crate::format::{Content, Mesh, Primitive, Snapshot, Texture, TextureKey, Vertex};

const USER_TEXTURE: u64 = 1 << 63;

#[derive(Default)]
pub struct TextureStore {
    images: HashMap<TextureKey, Image>,
}

struct Image {
    size: [u32; 2],
    pixels: Vec<[u8; 4]>,
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

    fn texture(&self, texture: TextureKey) -> Result<Texture, String> {
        let image = self
            .images
            .get(&texture)
            .ok_or("the painting uses a texture that was never uploaded")?;
        Texture::encode(image.size, &image.pixels)
    }
}

fn key(id: TextureId) -> TextureKey {
    match id {
        TextureId::Managed(id) => id,
        TextureId::User(id) => id | USER_TEXTURE,
    }
}

pub fn capture(
    context: &egui::Context,
    output: &egui::FullOutput,
    textures: &TextureStore,
) -> Result<Snapshot, String> {
    let pixels_per_point = output.pixels_per_point;
    let screen = context.content_rect();
    let tessellated = context.tessellate(output.shapes.clone(), pixels_per_point);

    let mut primitives = Vec::with_capacity(tessellated.len());
    for clipped in tessellated {
        let clip = rectangle(clipped.clip_rect);
        let content = match clipped.primitive {
            EguiPrimitive::Mesh(mesh) => Content::Mesh(Mesh {
                texture: key(mesh.texture_id),
                indices: mesh.indices,
                vertices: mesh
                    .vertices
                    .into_iter()
                    .map(|vertex| Vertex {
                        pos: [vertex.pos.x, vertex.pos.y],
                        uv: [vertex.uv.x, vertex.uv.y],
                        color: vertex.color.to_array(),
                    })
                    .collect(),
            }),
            EguiPrimitive::Callback(callback) => Content::Callback(rectangle(callback.rect)),
        };
        primitives.push(Primitive { clip, content });
    }

    let mut used = BTreeMap::new();
    for primitive in &primitives {
        if let Content::Mesh(mesh) = &primitive.content {
            if let std::collections::btree_map::Entry::Vacant(e) = used.entry(mesh.texture) {
                e.insert(textures.texture(mesh.texture)?);
            }
        }
    }

    Ok(Snapshot {
        size: [
            (screen.width() * pixels_per_point).round() as u32,
            (screen.height() * pixels_per_point).round() as u32,
        ],
        pixels_per_point,
        background: context.global_style().visuals.panel_fill.to_array(),
        primitives,
        textures: used,
    })
}

fn rectangle(rect: egui::Rect) -> [f32; 4] {
    [rect.min.x, rect.min.y, rect.max.x, rect.max.y]
}
