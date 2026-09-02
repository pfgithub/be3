use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, TryRecvError},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use block_plugin_api::{
    AssetResult, AudioCommand, AudioStatus, BlockPick, ChildId, ChildLayer, ChildMode,
    ChildPlacement, ChildRect, ChildStatus, ClipboardImage, EditorBand, EditorRegion, FetchResult,
    FilePick, Occluder, PerformanceMeasurement, ViewChange,
};
pub use block_plugin_api::{BlockFilter, FileFilter};
use block_ui::BlockCatalog;
use eframe::egui;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct BlockDrag {
    pub position: egui::Pos2,
    pub block_id: Uuid,
    pub block_type: Uuid,
    pub dropped: bool,
}

#[derive(Clone)]
pub struct FileDrop {
    pub position: egui::Pos2,
    pub files: Vec<PickedFile>,
    pub dropped: bool,
}

#[derive(Clone, Copy)]
pub struct Artifact {
    pub block_id: Uuid,
    pub block_type: Uuid,
}

pub struct ArtifactDescription {
    pub source: Uuid,
    pub summary: String,
}

#[derive(Clone)]
pub struct PickedFile {
    pub name: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Default)]
pub struct Waker(Option<Arc<dyn Fn() + Send + Sync>>);

impl Waker {
    pub fn wake(&self) {
        if let Some(wake) = &self.0 {
            wake();
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Some(Arc::new(wake)))
    }
}
pub struct Task<T> {
    receiver: Receiver<T>,
    result: Option<T>,
    done: bool,
}

impl<T: Send + 'static> Task<T> {
    fn spawn(waker: &Waker, future: impl std::future::Future<Output = T> + Send + 'static) -> Self {
        let (sender, receiver) = mpsc::channel();
        let waker = waker.clone();
        block_client::spawn(async move {
            let _ = sender.send(future.await);
            waker.wake();
        });
        Self {
            receiver,
            result: None,
            done: false,
        }
    }
}

impl<T> Task<T> {
    pub fn poll(&mut self) -> Option<&T> {
        if !self.done {
            match self.receiver.try_recv() {
                Ok(result) => {
                    self.result = Some(result);
                    self.done = true;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.done = true,
            }
        }
        self.result.as_ref()
    }

    pub fn take(&mut self) -> Option<T> {
        self.poll();
        self.result.take()
    }

    pub fn finished(&self) -> bool {
        self.done
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
struct PerformanceRecord {
    group: Arc<str>,
    measurement: PerformanceMeasurement,
}

#[derive(Clone)]
pub struct PerformanceReporter {
    group: Arc<str>,
    records: Arc<Mutex<Vec<PerformanceRecord>>>,
}

impl PerformanceReporter {
    pub fn measure(&self, name: impl Into<String>) -> PerformanceMeasurementGuard {
        PerformanceMeasurementGuard {
            reporter: self.clone(),
            name: name.into(),
            started: Instant::now(),
        }
    }

    pub fn record_duration(&self, name: impl Into<String>, duration: Duration) {
        self.record(PerformanceMeasurement::Duration {
            name: name.into(),
            nanoseconds: duration.as_nanos().min(u128::from(u64::MAX)) as u64,
        });
    }

    pub fn record_count(&self, name: impl Into<String>, count: u64) {
        self.record(PerformanceMeasurement::Count {
            name: name.into(),
            count,
        });
    }

    fn record(&self, measurement: PerformanceMeasurement) {
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(PerformanceRecord {
                group: Arc::clone(&self.group),
                measurement,
            });
    }
}

pub struct PerformanceMeasurementGuard {
    reporter: PerformanceReporter,
    name: String,
    started: Instant,
}

impl Drop for PerformanceMeasurementGuard {
    fn drop(&mut self) {
        self.reporter
            .record_duration(std::mem::take(&mut self.name), self.started.elapsed());
    }
}

#[derive(Clone, Copy, Default)]
struct Region {
    region: Option<EditorRegion>,
    origin: egui::Vec2,
}

type ChildKey = (EditorRegion, Uuid, u32);

#[derive(Default)]
struct Children {
    placements: Vec<ChildPlacement>,
    occluders: Vec<Occluder>,
    ordinals: HashMap<Uuid, u32>,
    identities: HashMap<ChildKey, ChildId>,
    used: Vec<ChildKey>,
    next: u64,
}

pub struct ChildHandle {
    host: EditorHost,
    index: usize,
    child: ChildId,
    painter: egui::Painter,
    shape: egui::layers::ShapeIdx,
    rect: egui::Rect,
    status: Option<ChildStatus>,
    pub response: egui::Response,
}

impl ChildHandle {
    pub fn id(&self) -> ChildId {
        self.child
    }

    pub fn rect(&self) -> egui::Rect {
        self.rect
    }

    pub fn available(&self) -> bool {
        self.status.as_ref().is_some_and(|status| status.available)
    }

    pub fn hovered(&self) -> bool {
        self.status.as_ref().is_some_and(|status| status.hovered)
    }

    pub fn active(&self) -> bool {
        self.status.as_ref().is_some_and(|status| status.active)
    }

    pub fn error(&self) -> Option<&str> {
        self.status.as_ref()?.error.as_deref()
    }

    pub fn intrinsic_size(&self) -> Option<egui::Vec2> {
        let status = self.status.as_ref()?;
        (status.intrinsic_width > 0.0 && status.intrinsic_height > 0.0)
            .then(|| egui::vec2(status.intrinsic_width, status.intrinsic_height))
    }

    pub fn aspect_ratio(&self) -> Option<f32> {
        let status = self.status.as_ref()?;
        (status.aspect_ratio > 0.0).then_some(status.aspect_ratio)
    }

    pub fn set_mode(&self, mode: ChildMode) {
        self.host.update_child(self.index, |placement| {
            placement.mode = mode;
        });
    }

    pub fn activate(&self) {
        self.set_mode(ChildMode::Active);
    }

    pub fn keep_active(&self) {
        self.set_mode(ChildMode::Live);
    }

    pub fn set_corner_radius(&self, radius: f32) {
        let layer = self.host.update_child(self.index, |placement| {
            placement.corner_radius = radius;
            placement.layer
        });
        let Some(layer) = layer else {
            return;
        };
        self.painter
            .set(self.shape, self.host.child_shape(self.rect, radius, layer));
    }
}

#[derive(Clone, Default)]
pub struct EditorHost {
    waker: Waker,
    opens: Rc<RefCell<Vec<(Uuid, Uuid)>>>,
    block_types: Rc<RefCell<Rc<BlockCatalog>>>,
    drag: Rc<Cell<Option<BlockDrag>>>,
    files: Rc<RefCell<Option<FileDrop>>>,
    drag_accepted: Rc<Cell<Option<bool>>>,
    picks: Rc<RefCell<Vec<(u64, FileFilter)>>>,
    picked: Rc<RefCell<HashMap<u64, FilePick>>>,
    next_pick: Rc<Cell<u64>>,
    editable: Rc<Cell<bool>>,
    client_id: Rc<Cell<Uuid>>,
    view: Rc<Cell<Option<egui::Rect>>>,
    view_changes: Rc<RefCell<Vec<ViewChange>>>,
    creation_ready: Rc<Cell<bool>>,
    creation_changed: Rc<Cell<bool>>,
    performance: Arc<Mutex<Vec<PerformanceRecord>>>,
    region: Rc<Cell<Region>>,
    children: Rc<RefCell<Children>>,
    child_statuses: Rc<RefCell<HashMap<ChildId, ChildStatus>>>,
    block_picks: Rc<RefCell<Vec<(u64, BlockFilter)>>>,
    blocks_picked: Rc<RefCell<HashMap<u64, BlockPick>>>,
    next_block_pick: Rc<Cell<u64>>,
    audio_commands: Rc<RefCell<Vec<(Uuid, AudioCommand)>>>,
    audio_status: Rc<RefCell<AudioStatus>>,
    pastes: Rc<RefCell<Vec<u64>>>,
    pasted: Rc<RefCell<HashMap<u64, ClipboardImage>>>,
    next_paste: Rc<Cell<u64>>,
    fetches: Rc<RefCell<Vec<(u64, String)>>>,
    fetched: Rc<RefCell<HashMap<u64, FetchResult>>>,
    next_fetch: Rc<Cell<u64>>,
    asset_reads: Rc<RefCell<Vec<(u64, String)>>>,
    assets: Rc<RefCell<HashMap<u64, AssetResult>>>,
    next_asset: Rc<Cell<u64>>,
    presenting: Rc<Cell<bool>>,
    present_requests: Rc<RefCell<Vec<bool>>>,
    hidden_bands: Rc<RefCell<HashSet<EditorBand>>>,
}

impl EditorHost {
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn new(waker: Waker) -> Self {
        Self {
            waker,
            ..Self::default()
        }
    }

    pub fn waker(&self) -> Waker {
        self.waker.clone()
    }

    pub fn spawn<T: Send + 'static>(
        &self,
        future: impl std::future::Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        Task::spawn(&self.waker, future)
    }
    pub fn performance(&self, group: impl Into<String>) -> PerformanceReporter {
        PerformanceReporter {
            group: Arc::from(group.into()),
            records: Arc::clone(&self.performance),
        }
    }

    pub fn open_block(&self, block_id: Uuid, block_type: Uuid) {
        self.opens.borrow_mut().push((block_id, block_type));
    }

    pub fn block_types(&self) -> Rc<BlockCatalog> {
        Rc::clone(&self.block_types.borrow())
    }

    pub fn editable(&self) -> bool {
        self.editable.get()
    }

    pub fn client_id(&self) -> Uuid {
        self.client_id.get()
    }

    pub fn view(&self) -> Option<egui::Rect> {
        let origin = self.region.get().origin;
        self.view.get().map(|view| view.translate(origin))
    }

    pub fn pan_view(&self, delta: egui::Vec2) {
        self.view_changes.borrow_mut().push(ViewChange::Pan {
            x: delta.x,
            y: delta.y,
        });
    }

    pub fn zoom_view(&self, factor: f32, anchor: Option<egui::Pos2>) {
        let origin = self.region.get().origin;
        self.view_changes.borrow_mut().push(ViewChange::Zoom {
            factor,
            anchor: anchor.map(|anchor| (anchor.x - origin.x, anchor.y - origin.y)),
        });
    }

    pub fn fit_view(&self) {
        self.view_changes.borrow_mut().push(ViewChange::Fit);
    }

    pub fn drag(&self) -> Option<BlockDrag> {
        self.drag.get()
    }

    pub fn files(&self) -> Option<FileDrop> {
        self.files.borrow().clone()
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_files(&self, drop: Option<FileDrop>) {
        *self.files.borrow_mut() = drop;
    }

    pub fn accept_drag(&self, accepted: bool) {
        self.drag_accepted.set(Some(accepted));
    }

    pub fn pick_file(&self, filter: FileFilter) -> u64 {
        let request = self.next_pick.get() + 1;
        self.next_pick.set(request);
        self.picks.borrow_mut().push((request, filter));
        request
    }

    pub fn take_pick(&self, request: u64) -> Option<FilePick> {
        self.picked.borrow_mut().remove(&request)
    }

    pub fn pick_block(&self, filter: BlockFilter) -> u64 {
        let request = self.next_block_pick.get() + 1;
        self.next_block_pick.set(request);
        self.block_picks.borrow_mut().push((request, filter));
        request
    }

    pub fn take_block_pick(&self, request: u64) -> Option<BlockPick> {
        self.blocks_picked.borrow_mut().remove(&request)
    }

    pub fn paste_image(&self) -> u64 {
        let request = self.next_paste.get() + 1;
        self.next_paste.set(request);
        self.pastes.borrow_mut().push(request);
        request
    }

    pub fn take_pasted_image(&self, request: u64) -> Option<ClipboardImage> {
        self.pasted.borrow_mut().remove(&request)
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_pastes(&self) -> Vec<u64> {
        std::mem::take(&mut self.pastes.borrow_mut())
    }

    pub fn set_pasted_image(&self, request: u64, image: ClipboardImage) {
        self.pasted.borrow_mut().insert(request, image);
    }

    pub fn play_audio(&self, block_id: Uuid) {
        self.audio_commands
            .borrow_mut()
            .push((block_id, AudioCommand::Toggle));
    }

    pub fn reset_audio(&self, block_id: Uuid) {
        self.audio_commands
            .borrow_mut()
            .push((block_id, AudioCommand::Reset));
    }

    pub fn audio(&self) -> AudioStatus {
        self.audio_status.borrow().clone()
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_audio_commands(&self) -> Vec<(Uuid, AudioCommand)> {
        std::mem::take(&mut self.audio_commands.borrow_mut())
    }

    pub fn set_audio(&self, status: AudioStatus) {
        *self.audio_status.borrow_mut() = status;
    }

    pub fn fetch(&self, url: impl Into<String>) -> u64 {
        let request = self.next_fetch.get() + 1;
        self.next_fetch.set(request);
        self.fetches.borrow_mut().push((request, url.into()));
        request
    }

    pub fn take_fetch(&self, request: u64) -> Option<FetchResult> {
        self.fetched.borrow_mut().remove(&request)
    }

    pub fn take_fetches(&self) -> Vec<(u64, String)> {
        std::mem::take(&mut self.fetches.borrow_mut())
    }

    pub fn set_fetched(&self, request: u64, result: FetchResult) {
        self.fetched.borrow_mut().insert(request, result);
    }

    pub fn read_asset(&self, name: impl Into<String>) -> u64 {
        let request = self.next_asset.get() + 1;
        self.next_asset.set(request);
        self.asset_reads.borrow_mut().push((request, name.into()));
        request
    }

    pub fn take_asset(&self, request: u64) -> Option<AssetResult> {
        self.assets.borrow_mut().remove(&request)
    }

    pub fn take_asset_reads(&self) -> Vec<(u64, String)> {
        std::mem::take(&mut self.asset_reads.borrow_mut())
    }

    pub fn set_asset(&self, request: u64, result: AssetResult) {
        self.assets.borrow_mut().insert(request, result);
    }

    pub fn child(&self, ui: &mut egui::Ui, block_id: Uuid, block_type: Uuid) -> ChildHandle {
        let size = ui.available_size_before_wrap();
        self.place_child(ui, size, block_id, block_type, ChildLayer::Below)
    }

    pub fn child_sized(
        &self,
        ui: &mut egui::Ui,
        size: egui::Vec2,
        block_id: Uuid,
        block_type: Uuid,
    ) -> ChildHandle {
        self.place_child(ui, size, block_id, block_type, ChildLayer::Below)
    }

    pub fn child_above(&self, ui: &mut egui::Ui, block_id: Uuid, block_type: Uuid) -> ChildHandle {
        let size = ui.available_size_before_wrap();
        self.place_child(ui, size, block_id, block_type, ChildLayer::Above)
    }

    pub fn show_band(&self, band: EditorBand, shown: bool) {
        let mut hidden = self.hidden_bands.borrow_mut();
        match shown {
            true => hidden.remove(&band),
            false => hidden.insert(band),
        };
    }

    pub fn band_shown(&self, band: EditorBand) -> bool {
        !self.hidden_bands.borrow().contains(&band)
    }

    pub fn presenting(&self) -> bool {
        self.presenting.get()
    }

    pub fn present(&self, presenting: bool) {
        if self.presenting.get() == presenting {
            return;
        }
        self.present_requests.borrow_mut().push(presenting);
    }

    pub fn occlude(&self, rect: egui::Rect) {
        let origin = self.region.get().origin;
        let mut children = self.children.borrow_mut();
        let after = children.placements.len() as u32;
        children.occluders.push(Occluder {
            after,
            rect: child_rect(rect.translate(-origin)),
        });
    }

    fn place_child(
        &self,
        ui: &mut egui::Ui,
        size: egui::Vec2,
        block_id: Uuid,
        block_type: Uuid,
        layer: ChildLayer,
    ) -> ChildHandle {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let painter = ui.painter().clone();
        let state = self.region.get();
        let region = state.region.unwrap_or(EditorRegion::Frame);
        let mut children = self.children.borrow_mut();
        let ordinal = {
            let ordinal = children.ordinals.entry(block_id).or_default();
            let current = *ordinal;
            *ordinal += 1;
            current
        };
        let key = (region, block_id, ordinal);
        let child = match children.identities.get(&key) {
            Some(child) => *child,
            None => {
                children.next += 1;
                let child = ChildId(children.next);
                children.identities.insert(key, child);
                child
            }
        };
        children.used.push(key);
        let shape = painter.add(self.child_shape(rect, 0.0, layer));
        let index = children.placements.len();
        children.placements.push(ChildPlacement {
            child,
            block_id: block_id.into_bytes(),
            block_type: block_type.into_bytes(),
            rect: child_rect(rect.translate(-state.origin)),
            clip: child_rect(ui.clip_rect().intersect(rect).translate(-state.origin)),
            corner_radius: 0.0,
            layer,
            mode: ChildMode::Passive,
        });
        drop(children);
        ChildHandle {
            host: self.clone(),
            index,
            child,
            painter,
            shape,
            rect,
            status: self.child_statuses.borrow().get(&child).cloned(),
            response,
        }
    }

    fn child_shape(&self, rect: egui::Rect, radius: f32, layer: ChildLayer) -> egui::Shape {
        match layer {
            ChildLayer::Below => punch_shape(rect, radius),
            ChildLayer::Above => egui::Shape::Noop,
        }
    }

    fn update_child<T>(
        &self,
        index: usize,
        edit: impl FnOnce(&mut ChildPlacement) -> T,
    ) -> Option<T> {
        Some(edit(self.children.borrow_mut().placements.get_mut(index)?))
    }

    pub fn set_creation_ready(&self, ready: bool) {
        if self.creation_ready.get() == ready {
            return;
        }
        self.creation_ready.set(ready);
        self.creation_changed.set(true);
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_opens(&self) -> Vec<(Uuid, Uuid)> {
        std::mem::take(&mut self.opens.borrow_mut())
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_block_types(&self, catalog: Rc<BlockCatalog>) {
        *self.block_types.borrow_mut() = catalog;
    }

    pub fn set_client_id(&self, client_id: Uuid) {
        self.client_id.set(client_id);
    }

    pub fn set_editable(&self, editable: bool) {
        self.editable.set(editable);
    }

    pub fn set_view(&self, view: egui::Rect) {
        self.view.set(Some(view));
    }

    pub fn take_view_changes(&self) -> Vec<ViewChange> {
        std::mem::take(&mut self.view_changes.borrow_mut())
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_drag(&self, drag: Option<BlockDrag>) {
        self.drag.set(drag);
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_drag_accepted(&self) -> Option<bool> {
        self.drag_accepted.take()
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_picks(&self) -> Vec<(u64, FileFilter)> {
        std::mem::take(&mut self.picks.borrow_mut())
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_pick(&self, request: u64, pick: FilePick) {
        self.picked.borrow_mut().insert(request, pick);
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_block_picks(&self) -> Vec<(u64, BlockFilter)> {
        std::mem::take(&mut self.block_picks.borrow_mut())
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_block_pick(&self, request: u64, pick: BlockPick) {
        self.blocks_picked.borrow_mut().insert(request, pick);
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn begin_region(&self, region: EditorRegion, origin: egui::Vec2) {
        self.region.set(Region {
            region: Some(region),
            origin,
        });
        let mut children = self.children.borrow_mut();
        children.placements.clear();
        children.occluders.clear();
        children.ordinals.clear();
        children.used.clear();
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn end_region(&self, region: EditorRegion) -> (Vec<ChildPlacement>, Vec<Occluder>) {
        self.region.set(Region::default());
        let mut children = self.children.borrow_mut();
        let used = std::mem::take(&mut children.used);
        children
            .identities
            .retain(|key, _| key.0 != region || used.contains(key));
        (
            std::mem::take(&mut children.placements),
            std::mem::take(&mut children.occluders),
        )
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_child_statuses(&self, statuses: Vec<ChildStatus>) {
        let mut current = self.child_statuses.borrow_mut();
        for status in statuses {
            current.insert(status.child, status);
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn retain_child_statuses(&self, live: &[ChildId]) {
        self.child_statuses
            .borrow_mut()
            .retain(|child, _| live.contains(child));
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_presenting(&self, presenting: bool) {
        self.presenting.set(presenting);
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_present_requests(&self) -> Vec<bool> {
        std::mem::take(&mut self.present_requests.borrow_mut())
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_creation_ready(&self) -> Option<bool> {
        self.creation_changed
            .take()
            .then(|| self.creation_ready.get())
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_performance(&self) -> Vec<(String, Vec<PerformanceMeasurement>)> {
        let records = std::mem::take(
            &mut *self
                .performance
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        let mut groups = std::collections::BTreeMap::<Arc<str>, Vec<PerformanceMeasurement>>::new();
        for record in records {
            groups
                .entry(record.group)
                .or_default()
                .push(record.measurement);
        }
        groups
            .into_iter()
            .map(|(group, measurements)| (group.to_string(), measurements))
            .collect()
    }
}

#[derive(Default)]
pub struct FilePicker {
    request: Option<u64>,
}

impl FilePicker {
    pub fn open(&mut self, host: &EditorHost, filter: FileFilter) {
        self.request = Some(host.pick_file(filter));
    }

    pub fn is_open(&self) -> bool {
        self.request.is_some()
    }

    pub fn poll(&mut self, host: &EditorHost) -> Option<Result<PickedFile, String>> {
        let pick = host.take_pick(self.request?)?;
        self.request = None;
        match pick {
            FilePick::Chosen { name, data } => Some(Ok(PickedFile { name, data })),
            FilePick::Cancelled => None,
            FilePick::Failed(error) => Some(Err(error)),
        }
    }
}

#[derive(Default)]
pub struct ImagePaster {
    request: Option<u64>,
}

impl ImagePaster {
    pub fn poll(&mut self, ui: &egui::Ui, host: &EditorHost, enabled: bool) -> Option<PastedImage> {
        if let Some(request) = self.request {
            return match host.take_pasted_image(request)? {
                ClipboardImage::Pasted { name, data } => {
                    self.request = None;
                    Some(PastedImage::Image { name, data })
                }
                ClipboardImage::Empty => {
                    self.request = None;
                    Some(PastedImage::Empty)
                }
                ClipboardImage::Failed(error) => {
                    self.request = None;
                    Some(PastedImage::Failed(error))
                }
            };
        }
        if !enabled || !pasted(ui) {
            return None;
        }
        self.request = Some(host.paste_image());
        None
    }
}

pub enum PastedImage {
    Image { name: String, data: Vec<u8> },
    Empty,
    Failed(String),
}

fn pasted(ui: &egui::Ui) -> bool {
    ui.input(|input| {
        input
            .raw
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::Paste(_)))
            || (input.modifiers.command && input.key_pressed(egui::Key::V))
    })
}

fn child_rect(rect: egui::Rect) -> ChildRect {
    ChildRect {
        x: rect.min.x,
        y: rect.min.y,
        width: rect.width().max(0.0),
        height: rect.height().max(0.0),
    }
}

fn punch_shape(rect: egui::Rect, radius: f32) -> egui::Shape {
    #[cfg(target_arch = "wasm32")]
    {
        crate::panes::punch(rect, radius)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (rect, radius);
        egui::Shape::Noop
    }
}

#[derive(Default)]
pub struct BlockPicker {
    request: Option<u64>,
}

impl BlockPicker {
    pub fn open(&mut self, host: &EditorHost, filter: BlockFilter) {
        self.request = Some(host.pick_block(filter));
    }

    pub fn is_open(&self) -> bool {
        self.request.is_some()
    }

    pub fn poll(&mut self, host: &EditorHost) -> Option<Result<(Uuid, Uuid), String>> {
        let pick = host.take_block_pick(self.request?)?;
        self.request = None;
        match pick {
            BlockPick::Chosen {
                block_id,
                block_type,
            } => Some(Ok((
                Uuid::from_bytes(block_id),
                Uuid::from_bytes(block_type),
            ))),
            BlockPick::Cancelled => None,
            BlockPick::Failed(error) => Some(Err(error)),
        }
    }
}

#[cfg(test)]
mod tests;
