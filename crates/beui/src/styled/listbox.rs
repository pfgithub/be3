use beui_macros::component;

use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{with_document, Prop};
use crate::styled::choice::{self, Kind};

#[component]
pub fn listbox(
    labels: Vec<String>,
    selected: Prop<Option<usize>>,
    on_change: Option<Handler<Option<usize>>>,
) -> NodeId {
    let mut on_change = on_change;
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let control =
        with_document(|document| choice::choice(document, &label_refs, None, Kind::Listbox));

    with_document(|document| {
        choice::set_on_change(document, control, move |document, selected| {
            if let Some(handler) = &mut on_change {
                handler(document, selected);
            }
        });
    });

    selected.apply(move |selected| {
        with_document(|document| choice::set_selected(document, control, selected));
    });

    control
}

pub fn listbox_selected(document: &Document, control: NodeId) -> Option<usize> {
    let inner = document.shadow_root(control);
    choice::selected_index(document, inner)
}

pub fn focus_listbox(document: &mut Document, control: NodeId) {
    let inner = document.shadow_root(control);
    choice::focus(document, inner);
}
