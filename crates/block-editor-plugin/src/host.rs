use std::{cell::RefCell, rc::Rc};

use uuid::Uuid;

/// An editor instance's way of asking the host for something the plugin
/// cannot do itself. Cloning it shares the same queue, so a copy kept on a
/// widget still reaches the instance it came from.
#[derive(Clone, Default)]
pub struct EditorHost {
    opens: Rc<RefCell<Vec<(Uuid, Uuid)>>>,
}

impl EditorHost {
    /// Asks the host to open `block_id` in a tab of its own.
    pub fn open_block(&self, block_id: Uuid, block_type: Uuid) {
        self.opens.borrow_mut().push((block_id, block_type));
    }

    #[cfg(any(target_arch = "wasm32", target_os = "windows"))]
    pub(crate) fn take_opens(&self) -> Vec<(Uuid, Uuid)> {
        std::mem::take(&mut self.opens.borrow_mut())
    }
}
