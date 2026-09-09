use beui_macros::component;

use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{with_document, Prop};
use crate::styled::choice::{self, Kind};
use crate::unstyled;

#[component]
pub fn radio_group(
    labels: Vec<String>,
    selected: Prop<Option<usize>>,
    on_change: Option<Handler<Option<usize>>>,
) -> NodeId {
    let mut on_change = on_change;
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let control = choice::choice(&label_refs, None, Kind::Radio, move |document, selected| {
        if let Some(handler) = &mut on_change {
            handler(document, selected);
        }
    });

    selected.apply(move |selected| {
        with_document(|document| unstyled::set_choice_selected(document, control, selected));
    });

    control
}

pub fn radio_group_selected(document: &Document, control: NodeId) -> Option<usize> {
    choice::selected_index(document, control)
}

pub fn focus_radio_group(control: NodeId) {
    choice::focus(control);
}
