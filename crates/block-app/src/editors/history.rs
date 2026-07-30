use std::collections::VecDeque;

const MAX_ACTIONS: usize = 100;
const MAX_BYTES: usize = 64 * 1024 * 1024;

struct Entry<T> {
    action: T,
    bytes: usize,
}

pub(super) struct History<T> {
    undo: VecDeque<Entry<T>>,
    redo: Vec<Entry<T>>,
    undo_bytes: usize,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            undo_bytes: 0,
        }
    }
}

impl<T> History<T> {
    pub fn push(&mut self, action: T, bytes: usize) {
        self.redo.clear();
        self.undo_bytes = self.undo_bytes.saturating_add(bytes);
        self.undo.push_back(Entry { action, bytes });
        while self.undo.len() > MAX_ACTIONS || self.undo_bytes > MAX_BYTES {
            if self.undo.len() == 1 {
                break;
            }
            if let Some(entry) = self.undo.pop_front() {
                self.undo_bytes = self.undo_bytes.saturating_sub(entry.bytes);
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn last_undo_mut(&mut self) -> Option<&mut T> {
        if self.redo.is_empty() {
            self.undo.back_mut().map(|entry| &mut entry.action)
        } else {
            None
        }
    }

    pub fn last_undo(&self) -> Option<&T> {
        if self.redo.is_empty() {
            self.undo.back().map(|entry| &entry.action)
        } else {
            None
        }
    }

    pub fn undo(&mut self, apply: impl FnOnce(&mut T)) {
        let Some(mut entry) = self.undo.pop_back() else {
            return;
        };
        self.undo_bytes = self.undo_bytes.saturating_sub(entry.bytes);
        apply(&mut entry.action);
        self.redo.push(entry);
    }

    pub fn redo(&mut self, apply: impl FnOnce(&mut T)) {
        let Some(mut entry) = self.redo.pop() else {
            return;
        };
        apply(&mut entry.action);
        self.undo_bytes = self.undo_bytes.saturating_add(entry.bytes);
        self.undo.push_back(entry);
    }
}

#[cfg(test)]
#[path = "history/tests/history_evicts_oldest_entries.rs"]
mod history_evicts_oldest_entries;
#[cfg(test)]
#[path = "history/tests/history_new_action_clears_redo.rs"]
mod history_new_action_clears_redo;
#[cfg(test)]
#[path = "history/tests/history_replays_in_stack_order.rs"]
mod history_replays_in_stack_order;
