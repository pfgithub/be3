use std::collections::HashMap;

use block_gpu_abi::Handle;

pub(crate) struct Table<T> {
    entries: HashMap<Handle, T>,
    next: Handle,
}

impl<T> Table<T> {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            next: 1,
        }
    }

    pub(crate) fn insert(&mut self, value: T) -> Handle {
        let handle = self.next;
        self.next = self.next.wrapping_add(1).max(1);
        self.entries.insert(handle, value);
        handle
    }

    pub(crate) fn get(&self, handle: Handle, kind: &str) -> Result<&T, String> {
        self.entries
            .get(&handle)
            .ok_or_else(|| format!("no {kind} is registered as handle {handle}"))
    }

    pub(crate) fn get_mut(&mut self, handle: Handle, kind: &str) -> Result<&mut T, String> {
        self.entries
            .get_mut(&handle)
            .ok_or_else(|| format!("no {kind} is registered as handle {handle}"))
    }

    pub(crate) fn take(&mut self, handle: Handle, kind: &str) -> Result<T, String> {
        self.entries
            .remove(&handle)
            .ok_or_else(|| format!("no {kind} is registered as handle {handle}"))
    }

    pub(crate) fn remove(&mut self, handle: Handle) {
        self.entries.remove(&handle);
    }
}
