pub mod infinite_canvas;
pub mod text;
pub mod workspace_index;

#[cfg(test)]
mod text_operations_are_crdt_updates_and_do_not_keep_a_confirmed_copy;
#[cfg(test)]
mod workspace_index_remove_removes_entry;
