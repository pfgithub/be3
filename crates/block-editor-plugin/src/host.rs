use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
};

pub use block_plugin_api::FileFilter;
use block_plugin_api::{FilePick, ViewChange};
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

#[derive(Clone, Copy)]
pub struct Artifact {
    pub block_id: Uuid,
    pub block_type: Uuid,
}

pub struct ArtifactDescription {
    pub source: Uuid,
    pub summary: String,
}

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

    pub(crate) fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Some(Arc::new(wake)))
    }
}

#[derive(Clone, Default)]
pub struct EditorHost {
    waker: Waker,
    opens: Rc<RefCell<Vec<(Uuid, Uuid)>>>,
    block_types: Rc<RefCell<Rc<BlockCatalog>>>,
    drag: Rc<Cell<Option<BlockDrag>>>,
    drag_accepted: Rc<Cell<Option<bool>>>,
    picks: Rc<RefCell<Vec<(u64, FileFilter)>>>,
    picked: Rc<RefCell<HashMap<u64, FilePick>>>,
    next_pick: Rc<Cell<u64>>,
    editable: Rc<Cell<bool>>,
    view: Rc<Cell<Option<egui::Rect>>>,
    view_changes: Rc<RefCell<Vec<ViewChange>>>,
    creation_ready: Rc<Cell<bool>>,
    creation_changed: Rc<Cell<bool>>,
}

impl EditorHost {
    #[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
    pub(crate) fn new(waker: Waker) -> Self {
        Self {
            waker,
            ..Self::default()
        }
    }

    pub fn waker(&self) -> Waker {
        self.waker.clone()
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

    pub fn view(&self) -> Option<egui::Rect> {
        self.view.get()
    }

    pub fn pan_view(&self, delta: egui::Vec2) {
        self.view_changes.borrow_mut().push(ViewChange::Pan {
            x: delta.x,
            y: delta.y,
        });
    }

    pub fn zoom_view(&self, factor: f32, anchor: Option<egui::Pos2>) {
        self.view_changes.borrow_mut().push(ViewChange::Zoom {
            factor,
            anchor: anchor.map(|anchor| (anchor.x, anchor.y)),
        });
    }

    pub fn fit_view(&self) {
        self.view_changes.borrow_mut().push(ViewChange::Fit);
    }

    pub fn drag(&self) -> Option<BlockDrag> {
        self.drag.get()
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

    pub fn set_creation_ready(&self, ready: bool) {
        if self.creation_ready.get() == ready {
            return;
        }
        self.creation_ready.set(ready);
        self.creation_changed.set(true);
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
    pub(crate) fn take_opens(&self) -> Vec<(Uuid, Uuid)> {
        std::mem::take(&mut self.opens.borrow_mut())
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
    pub(crate) fn set_block_types(&self, catalog: Rc<BlockCatalog>) {
        *self.block_types.borrow_mut() = catalog;
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
    pub(crate) fn set_editable(&self, editable: bool) {
        self.editable.set(editable);
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
    pub(crate) fn set_view(&self, view: egui::Rect) {
        self.view.set(Some(view));
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
    pub(crate) fn take_view_changes(&self) -> Vec<ViewChange> {
        std::mem::take(&mut self.view_changes.borrow_mut())
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
    pub(crate) fn set_drag(&self, drag: Option<BlockDrag>) {
        self.drag.set(drag);
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
    pub(crate) fn take_drag_accepted(&self) -> Option<bool> {
        self.drag_accepted.take()
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
    pub(crate) fn take_picks(&self) -> Vec<(u64, FileFilter)> {
        std::mem::take(&mut self.picks.borrow_mut())
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
    pub(crate) fn set_pick(&self, request: u64, pick: FilePick) {
        self.picked.borrow_mut().insert(request, pick);
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
    pub(crate) fn take_creation_ready(&self) -> Option<bool> {
        self.creation_changed
            .take()
            .then(|| self.creation_ready.get())
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
