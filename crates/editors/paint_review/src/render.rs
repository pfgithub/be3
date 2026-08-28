use std::sync::mpsc::Receiver;

use block_editor_plugin::{egui, Waker};
use paint_snapshot::Snapshot;

#[cfg(any(target_arch = "wasm32", test))]
mod inline;
#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(test, allow(dead_code))]
mod worker;

#[cfg(any(target_arch = "wasm32", test))]
use inline::start as start_raster;
#[cfg(all(not(target_arch = "wasm32"), not(test)))]
use worker::start as start_raster;

const CACHED: usize = 8;

pub struct Painted {
    pub image: egui::ColorImage,
    pub description: String,
}

pub struct Rendered {
    pub texture: egui::TextureHandle,
    pub size: egui::Vec2,
    pub description: String,
}

struct Raster {
    hash: String,
    painted: Receiver<Result<Painted, String>>,
}

#[derive(Default)]
pub struct Paintings {
    cached: Vec<(String, Result<Rendered, String>)>,
    rastering: Option<Raster>,
    #[cfg(test)]
    rasters: usize,
}

impl Paintings {
    pub fn rendered(
        &mut self,
        context: &egui::Context,
        hash: &str,
    ) -> Option<&Result<Rendered, String>> {
        self.settle(context);
        let held = self.cached.iter().position(|(seen, _)| seen == hash)?;
        let entry = self.cached.remove(held);
        self.cached.push(entry);
        self.cached.last().map(|(_, rendered)| rendered)
    }

    pub fn start(&mut self, hash: &str, data: Vec<u8>, waker: Waker) {
        if self.rastering.is_some() || self.cached.iter().any(|(seen, _)| seen == hash) {
            return;
        }
        #[cfg(test)]
        {
            self.rasters += 1;
        }
        self.rastering = Some(Raster {
            hash: hash.to_owned(),
            painted: start_raster(data, waker),
        });
    }

    #[cfg(test)]
    pub fn rasters(&self) -> usize {
        self.rasters
    }

    fn settle(&mut self, context: &egui::Context) {
        let Some(raster) = &self.rastering else {
            return;
        };
        let Ok(painted) = raster.painted.try_recv() else {
            return;
        };
        let hash = raster.hash.clone();
        self.rastering = None;
        self.hold(context, hash, painted);
    }

    fn hold(&mut self, context: &egui::Context, hash: String, painted: Result<Painted, String>) {
        let rendered = painted.map(|painted| Rendered {
            size: egui::vec2(painted.image.width() as f32, painted.image.height() as f32),
            texture: context.load_texture(
                "paint-review",
                painted.image,
                egui::TextureOptions::LINEAR,
            ),
            description: painted.description,
        });
        self.cached.push((hash, rendered));
        while self.cached.len() > CACHED {
            drop(self.cached.remove(0));
        }
    }
}

pub fn describe(snapshot: &Snapshot) -> String {
    let [width, height] = snapshot.size;
    format!(
        "{width}x{height}, {} draw calls, {} textures",
        snapshot.primitives.len(),
        snapshot.textures.len()
    )
}

pub fn change(approved: &[u8], current: &[u8]) -> Result<String, String> {
    let (approved, current) = (Snapshot::decode(approved)?, Snapshot::decode(current)?);
    Ok(paint_snapshot::difference(&approved, &current).map_or_else(
        || "the painting is the same".to_owned(),
        |difference| difference.description,
    ))
}

fn paint(data: &[u8]) -> Result<Painted, String> {
    let snapshot = Snapshot::decode(data)?;
    let image = paint_snapshot::render(&snapshot)?;
    Ok(Painted {
        image: egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        ),
        description: describe(&snapshot),
    })
}
