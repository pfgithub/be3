use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::mpsc::{channel, Receiver, Sender};

use block_editor_plugin::{EditorHost, FetchResult, Waker};

use crate::{mvt, raster};

pub(crate) const SOURCE_MAX_ZOOM: u8 = 14;
const TILE_URL_BASE: &str = "https://vector.openstreetmap.org/shortbread_v1";
const MAX_TILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_IN_FLIGHT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TileId {
    pub zoom: u8,
    pub x: u32,
    pub y: u32,
}

impl TileId {
    pub(crate) fn parent(self) -> Option<TileId> {
        self.zoom.checked_sub(1).map(|zoom| TileId {
            zoom,
            x: self.x / 2,
            y: self.y / 2,
        })
    }

    fn url(self) -> String {
        format!("{TILE_URL_BASE}/{}/{}/{}.mvt", self.zoom, self.x, self.y)
    }
}

pub(crate) struct TileResult {
    pub id: TileId,
    pub result: Result<raster::TileRaster, String>,
}

pub(crate) struct TileWorker {
    downloads: HashMap<u64, TileId>,
    queued: Vec<TileId>,
    requested: HashSet<TileId>,
    rasterizing: Sender<(TileId, Vec<u8>)>,
    rasterized: Receiver<TileResult>,
}

impl TileWorker {
    pub(crate) fn spawn(waker: Waker) -> Self {
        let (rasterizing, work) = channel();
        let (results, rasterized) = channel();
        let _ = std::thread::Builder::new()
            .name("map-tile-rasterizer".into())
            .spawn(move || rasterize(work, results, waker));
        Self {
            downloads: HashMap::new(),
            queued: Vec::new(),
            requested: HashSet::new(),
            rasterizing,
            rasterized,
        }
    }

    pub(crate) fn request(&mut self, id: TileId) {
        if !self.requested.insert(id) {
            return;
        }
        self.queued.push(id);
    }

    pub(crate) fn poll(&mut self, host: &EditorHost) -> Vec<TileResult> {
        while self.downloads.len() < MAX_IN_FLIGHT {
            let Some(id) = self.queued.pop() else {
                break;
            };
            self.downloads.insert(host.fetch(id.url()), id);
        }
        let mut failures = Vec::new();
        let answered: Vec<u64> = self.downloads.keys().copied().collect();
        for request in answered {
            let Some(result) = host.take_fetch(request) else {
                continue;
            };
            let id = self
                .downloads
                .remove(&request)
                .expect("the request was answered");
            match result {
                FetchResult::Body(body) => {
                    let _ = self.rasterizing.send((id, body));
                }
                FetchResult::Failed(error) => failures.push(TileResult {
                    id,
                    result: Err(error),
                }),
            }
        }
        while let Ok(result) = self.rasterized.try_recv() {
            failures.push(result);
        }
        failures
    }
}

fn rasterize(work: Receiver<(TileId, Vec<u8>)>, results: Sender<TileResult>, waker: Waker) {
    while let Ok((id, body)) = work.recv() {
        let result = decode(body).map(|tile| raster::rasterize(&tile, id.zoom));
        if results.send(TileResult { id, result }).is_err() {
            return;
        }
        waker.wake();
    }
}

fn decode(body: Vec<u8>) -> Result<mvt::Tile, String> {
    let mut data = body;
    if data.starts_with(&[0x1f, 0x8b]) {
        let mut decompressed = Vec::new();
        flate2::read::GzDecoder::new(data.as_slice())
            .take(MAX_TILE_BYTES)
            .read_to_end(&mut decompressed)
            .map_err(|error| format!("tile decompression failed: {error}"))?;
        data = decompressed;
    }
    mvt::decode(&data)
}

#[cfg(test)]
mod tests;
