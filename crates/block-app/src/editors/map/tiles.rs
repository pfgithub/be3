//! Background download and rasterization of OpenStreetMap vector tiles.
//!
//! Tiles come from the OSMF vector tile service (Shortbread schema); see
//! <https://operations.osmfoundation.org/policies/vector/>.

use std::collections::HashSet;
use std::io::Read;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use eframe::egui;

use super::{mvt, raster};

/// The OSMF vector tile service serves tiles up to this zoom level.
pub(super) const SOURCE_MAX_ZOOM: u8 = 14;
const TILE_URL_BASE: &str = "https://vector.openstreetmap.org/shortbread_v1";
const USER_AGENT: &str = "be3-block-app/0.1 (map block editor)";
const MAX_TILE_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct TileId {
    pub zoom: u8,
    pub x: u32,
    pub y: u32,
}

impl TileId {
    pub(super) fn parent(self) -> Option<TileId> {
        self.zoom.checked_sub(1).map(|zoom| TileId {
            zoom,
            x: self.x / 2,
            y: self.y / 2,
        })
    }
}

pub(super) struct TileResult {
    pub id: TileId,
    pub result: Result<raster::TileRaster, String>,
}

pub(super) struct TileWorker {
    requests: Sender<TileId>,
    results: Receiver<TileResult>,
}

impl TileWorker {
    pub(super) fn spawn(context: egui::Context) -> Self {
        let (requests, request_receiver) = channel();
        let (result_sender, results) = channel();
        let _ = std::thread::Builder::new()
            .name("map-tile-worker".into())
            .spawn(move || worker(request_receiver, result_sender, context));
        Self { requests, results }
    }

    pub(super) fn request(&self, id: TileId) {
        let _ = self.requests.send(id);
    }

    pub(super) fn try_result(&self) -> Option<TileResult> {
        self.results.try_recv().ok()
    }
}

/// Pending tile requests, served most recent first so the tiles currently in
/// view take priority over any backlog from earlier views. Each tile is
/// served at most once; re-requesting a pending tile moves it back to the
/// top.
struct RequestQueue {
    pending: Vec<TileId>,
    served: HashSet<TileId>,
}

impl RequestQueue {
    fn new() -> Self {
        Self {
            pending: Vec::new(),
            served: HashSet::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    fn enqueue(&mut self, id: TileId) {
        if self.served.contains(&id) {
            return;
        }
        self.pending.retain(|pending| *pending != id);
        self.pending.push(id);
    }

    fn pop(&mut self) -> Option<TileId> {
        let id = self.pending.pop()?;
        self.served.insert(id);
        Some(id)
    }
}

fn worker(requests: Receiver<TileId>, results: Sender<TileResult>, context: egui::Context) {
    let agent = ureq::AgentBuilder::new()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build();
    let mut queue = RequestQueue::new();
    loop {
        if queue.is_empty() {
            match requests.recv() {
                Ok(id) => queue.enqueue(id),
                Err(_) => return,
            }
        }
        while let Ok(id) = requests.try_recv() {
            queue.enqueue(id);
        }
        let Some(id) = queue.pop() else {
            continue;
        };
        // A panic must fail the one tile, not silently kill the worker.
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| load_tile(&agent, id)))
                .unwrap_or_else(|_| {
                    Err(format!(
                        "tile {}/{}/{} rendering panicked",
                        id.zoom, id.x, id.y
                    ))
                });
        if results.send(TileResult { id, result }).is_err() {
            return;
        }
        context.request_repaint();
    }
}

fn load_tile(agent: &ureq::Agent, id: TileId) -> Result<raster::TileRaster, String> {
    let data = fetch(agent, id)?;
    let tile = mvt::decode(&data)?;
    Ok(raster::rasterize(&tile, id.zoom))
}

fn fetch(agent: &ureq::Agent, id: TileId) -> Result<Vec<u8>, String> {
    let url = format!("{TILE_URL_BASE}/{}/{}/{}.mvt", id.zoom, id.x, id.y);
    let response = agent
        .get(&url)
        .call()
        .map_err(|error| format!("tile download failed: {error}"))?;
    let mut data = Vec::new();
    response
        .into_reader()
        .take(MAX_TILE_BYTES)
        .read_to_end(&mut data)
        .map_err(|error| format!("tile download failed: {error}"))?;
    if data.starts_with(&[0x1f, 0x8b]) {
        let mut decompressed = Vec::new();
        flate2::read::GzDecoder::new(data.as_slice())
            .take(MAX_TILE_BYTES)
            .read_to_end(&mut decompressed)
            .map_err(|error| format!("tile decompression failed: {error}"))?;
        data = decompressed;
    }
    Ok(data)
}

#[cfg(test)]
mod tests;
