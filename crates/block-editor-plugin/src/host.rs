use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

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

/// An editor instance's way of asking the host for something the plugin
/// cannot do itself. Cloning it shares the same queue, so a copy kept on a
/// widget still reaches the instance it came from.
#[derive(Clone, Default)]
pub struct EditorHost {
    opens: Rc<RefCell<Vec<(Uuid, Uuid)>>>,
    block_types: Rc<RefCell<Rc<BlockCatalog>>>,
    drag: Rc<Cell<Option<BlockDrag>>>,
    drag_accepted: Rc<Cell<Option<bool>>>,
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

    /// The block being dragged over the region currently drawing, if any.
    pub fn drag(&self) -> Option<BlockDrag> {
        self.drag.get()
    }

    /// Answers the current drag, which decides the cursor the host shows.
    pub fn accept_drag(&self, accepted: bool) {
        self.drag_accepted.set(Some(accepted));
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
    pub(crate) fn set_drag(&self, drag: Option<BlockDrag>) {
        self.drag.set(drag);
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows"))]
    pub(crate) fn take_drag_accepted(&self) -> Option<bool> {
        self.drag_accepted.take()
    }
}
