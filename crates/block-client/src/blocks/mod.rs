pub mod infinite_canvas;
pub mod text;
pub mod web_browser_tab;
pub mod workspace_index;

#[cfg(test)]
mod text_operations_are_crdt_updates_and_do_not_keep_a_confirmed_copy;
#[cfg(test)]
mod web_browser_tab_history_changes_current_index;
#[cfg(test)]
mod web_browser_tab_new_starts_at_about_blank;
#[cfg(test)]
mod web_browser_tab_push_appends_and_selects_url;
#[cfg(test)]
mod web_browser_tab_push_discards_forward_history;
#[cfg(test)]
mod web_browser_tab_replace_changes_current_url;
#[cfg(test)]
mod web_browser_tab_title_determines_implicit_name;
#[cfg(test)]
mod workspace_index_remove_removes_entry;
