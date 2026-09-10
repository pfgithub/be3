use beui_macros::component;

use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{Callback, Prop};
use crate::styled::choice::{self, Kind};

#[component]
pub fn tabs(labels: Vec<String>, selected: Prop<usize>, on_change: Callback<usize>) -> NodeId {
    let option_count = labels.len();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let selected = selected.map(move |selected| Some(selected.min(option_count.saturating_sub(1))));
    choice::choice(
        &label_refs,
        selected,
        Kind::Tabs,
        Callback::new(move |selected| {
            if let Some(selected) = selected {
                on_change.call(selected);
            }
        }),
    )
}

pub fn tabs_selected(document: &Document, tabs: NodeId) -> usize {
    choice::selected_index(document, tabs).unwrap_or(0)
}

pub fn focus_tabs(tabs: NodeId) {
    choice::focus(tabs);
}
