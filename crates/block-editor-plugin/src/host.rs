use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

pub use block_plugin_api::FileFilter;
use block_plugin_api::FilePick;
use block_ui::BlockCatalog;
use eframe::egui;
use uuid::Uuid;

/// A block the host is dragging over the region being drawn, positioned in
/// that region's own coordinates.
#[derive(Clone, Copy)]
pub struct BlockDrag {
    pub position: egui::Pos2,
    pub block_id: Uuid,
    pub block_type: Uuid,
    pub dropped: bool,
}

pub struct PickedFile {
    pub name: String,
    pub data: Vec<u8>,
}

/// An editor instance's way of asking the host for something the plugin
/// cannot do itself. Cloning it shares the same queue, so a copy kept on a
/// widget still reaches the instance it came from.
#[derive(Clone, Default)]
pub struct EditorHost {
    opens: Rc<RefCell<Vec<(Uuid, Uuid)>>>,
    block_types: Rc<RefCell<Rc<BlockCatalog>>>,
    drag: Rc<Cell<Option<BlockDrag>>>,
    drag_accepted: Rc<Cell<Option<bool>>>,
    picks: Rc<RefCell<Vec<(u64, FileFilter)>>>,
    picked: Rc<RefCell<HashMap<u64, FilePick>>>,
    next_pick: Rc<Cell<u64>>,
    editable: Rc<Cell<bool>>,
    creation_ready: Rc<Cell<bool>>,
    creation_changed: Rc<Cell<bool>>,
}

impl EditorHost {
    /// Asks the host to open `block_id` in a tab of its own.
    pub fn open_block(&self, block_id: Uuid, block_type: Uuid) {
        self.opens.borrow_mut().push((block_id, block_type));
    }

    /// The block types the host has registered, for naming and illustrating
    /// blocks this editor only holds a reference to.
    pub fn block_types(&self) -> Rc<BlockCatalog> {
        Rc::clone(&self.block_types.borrow())
    }

    /// Whether the block this instance was opened on may be edited. An
    /// instance filling in a new block always may.
    pub fn editable(&self) -> bool {
        self.editable.get()
    }

    /// The block being dragged over the region currently drawing, if any.
    pub fn drag(&self) -> Option<BlockDrag> {
        self.drag.get()
    }

    /// Answers the current drag, which decides the cursor the host shows.
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

    #[cfg(any(target_arch = "wasm32", target_os = "windows"))]
    pub(crate) fn take_opens(&self) -> Vec<(Uuid, Uuid)> {
        std::mem::take(&mut self.opens.borrow_mut())
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows"))]
    pub(crate) fn set_block_types(&self, catalog: Rc<BlockCatalog>) {
        *self.block_types.borrow_mut() = catalog;
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows"))]
    pub(crate) fn set_editable(&self, editable: bool) {
        self.editable.set(editable);
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows"))]
    pub(crate) fn set_drag(&self, drag: Option<BlockDrag>) {
        self.drag.set(drag);
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows"))]
    pub(crate) fn take_drag_accepted(&self) -> Option<bool> {
        self.drag_accepted.take()
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows"))]
    pub(crate) fn take_picks(&self) -> Vec<(u64, FileFilter)> {
        std::mem::take(&mut self.picks.borrow_mut())
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows"))]
    pub(crate) fn set_pick(&self, request: u64, pick: FilePick) {
        self.picked.borrow_mut().insert(request, pick);
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows"))]
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
