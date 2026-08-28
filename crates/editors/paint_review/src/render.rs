use std::sync::mpsc::Receiver;

use block_editor_plugin::{egui, Waker};
use paint_snapshot::{Content, Snapshot};

#[cfg(any(target_arch = "wasm32", test))]
mod inline;
#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(test, allow(dead_code))]
mod worker;

#[cfg(any(target_arch = "wasm32", test))]
use inline::start as start_raster;
#[cfg(all(not(target_arch = "wasm32"), not(test)))]
use worker::start as start_raster;

const CACHED: usize = 16;

pub struct Painted {
    pub image: egui::ColorImage,
    pub description: String,
}

pub struct Rendered {
    pub texture: egui::TextureHandle,
    pub size: egui::Vec2,
    pub description: String,
}

pub struct Change {
    pub description: String,
    pub frame: Option<usize>,
}

struct Raster {
    frame: (String, usize),
    painted: Receiver<Result<Painted, String>>,
}

#[derive(Default)]
pub struct Paintings {
    cached: Vec<((String, usize), Result<Rendered, String>)>,
    rastering: Option<Raster>,
    #[cfg(test)]
    rasters: usize,
}

impl Paintings {
    pub fn rendered(
        &mut self,
        context: &egui::Context,
        hash: &str,
        frame: usize,
    ) -> Option<&Result<Rendered, String>> {
        self.settle(context);
        let held = self.held(hash, frame)?;
        let entry = self.cached.remove(held);
        self.cached.push(entry);
        self.cached.last().map(|(_, rendered)| rendered)
    }

    pub fn start(&mut self, hash: &str, frame: usize, data: Vec<u8>, waker: Waker) {
        if self.rastering.is_some() || self.held(hash, frame).is_some() {
            return;
        }
        #[cfg(test)]
        {
            self.rasters += 1;
        }
        self.rastering = Some(Raster {
            frame: (hash.to_owned(), frame),
            painted: start_raster(data, frame, waker),
        });
    }

    #[cfg(test)]
    pub fn rasters(&self) -> usize {
        self.rasters
    }

    fn held(&self, hash: &str, frame: usize) -> Option<usize> {
        self.cached
            .iter()
            .position(|((seen, at), _)| seen == hash && *at == frame)
    }

    fn settle(&mut self, context: &egui::Context) {
        let Some(raster) = &self.rastering else {
            return;
        };
        let Ok(painted) = raster.painted.try_recv() else {
            return;
        };
        let frame = raster.frame.clone();
        self.rastering = None;
        self.hold(context, frame, painted);
    }

    fn hold(
        &mut self,
        context: &egui::Context,
        frame: (String, usize),
        painted: Result<Painted, String>,
    ) {
        let rendered = painted.map(|painted| Rendered {
            size: egui::vec2(painted.image.width() as f32, painted.image.height() as f32),
            texture: context.load_texture(
                "paint-review",
                painted.image,
                egui::TextureOptions::LINEAR,
            ),
            description: painted.description,
        });
        self.cached.push((frame, rendered));
        while self.cached.len() > CACHED {
            drop(self.cached.remove(0));
        }
    }
}

pub fn frames(data: &[u8]) -> Result<usize, String> {
    Ok(Snapshot::decode(data)?.frames.len())
}

pub fn describe(snapshot: &Snapshot, frame: usize) -> Result<String, String> {
    let frame = snapshot.frame(frame)?;
    let [width, height] = frame.size;
    let textures: std::collections::BTreeSet<_> = frame
        .primitives
        .iter()
        .filter_map(|primitive| match &primitive.content {
            Content::Mesh(triangles) => Some(triangles),
            Content::Callback(_) => None,
        })
        .flatten()
        .map(|triangle| triangle.texture)
        .collect();
    Ok(format!(
        "{width}x{height}, {} draw calls, {} textures",
        frame.primitives.len(),
        textures.len()
    ))
}

pub fn change(approved: &[u8], current: &[u8]) -> Result<Change, String> {
    let (approved, current) = (Snapshot::decode(approved)?, Snapshot::decode(current)?);
    Ok(match paint_snapshot::difference(&approved, &current) {
        None => Change {
            description: "the painting is the same".to_owned(),
            frame: None,
        },
        Some(difference) => Change {
            description: difference.description,
            frame: difference.frame,
        },
    })
}

fn paint(data: &[u8], frame: usize) -> Result<Painted, String> {
    let snapshot = Snapshot::decode(data)?;
    let image = paint_snapshot::render(&snapshot, frame)?;
    Ok(Painted {
        image: egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        ),
        description: describe(&snapshot, frame)?,
    })
}
