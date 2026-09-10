use beui_macros::component;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{Callback, Prop};
use crate::styled::choice::{self, Kind};

#[component]
pub fn radio_group(
    labels: Vec<String>,
    selected: Prop<Option<usize>>,
    on_change: Callback<Option<usize>>,
) -> NodeId {
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    choice::choice(&label_refs, selected, Kind::Radio, on_change)
}

pub fn radio_group_selected(document: &Document, control: NodeId) -> Option<usize> {
    choice::selected_index(document, control)
}

pub fn focus_radio_group(control: NodeId) {
    choice::focus(control);
}
