use beui_macros::component;

use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::Prop;
use crate::styled::choice::{self, Kind};

#[component]
pub fn listbox(
    labels: Vec<String>,
    selected: Prop<Option<usize>>,
    on_change: Option<Handler<Option<usize>>>,
) -> NodeId {
    let mut on_change = on_change;
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    choice::choice(
        &label_refs,
        selected,
        Kind::Listbox,
        move |document, selected| {
            if let Some(handler) = &mut on_change {
                handler(document, selected);
            }
        },
    )
}

pub fn listbox_selected(document: &Document, control: NodeId) -> Option<usize> {
    choice::selected_index(document, control)
}

pub fn focus_listbox(control: NodeId) {
    choice::focus(control);
}
