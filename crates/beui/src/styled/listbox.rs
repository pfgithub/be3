use beui_macros::component;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{Callback, Prop};
use crate::styled::choice::{self, Kind};

#[component]
pub fn listbox(
    labels: Vec<String>,
    selected: Prop<Option<usize>>,
    on_change: Callback<Option<usize>>,
) -> NodeId {
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    choice::choice(&label_refs, selected, Kind::Listbox, on_change)
}

pub fn listbox_selected(document: &Document, control: NodeId) -> Option<usize> {
    choice::selected_index(document, control)
}

pub fn focus_listbox(control: NodeId) {
    choice::focus(control);
}
