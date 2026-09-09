use beui_macros::component;

use crate::document::Document;
use crate::node::{Handler, NodeId};
use crate::reactive::{with_document, Prop};
use crate::styled::choice::{self, Kind};

#[component]
pub fn tabs(
    labels: Vec<String>,
    selected: Prop<usize>,
    on_change: Option<Handler<usize>>,
) -> NodeId {
    let mut on_change = on_change;
    let option_count = labels.len();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let tabs = with_document(|document| choice::choice(document, &label_refs, None, Kind::Tabs));

    with_document(|document| {
        choice::set_on_change(document, tabs, move |document, selected| {
            if let (Some(selected), Some(handler)) = (selected, &mut on_change) {
                handler(document, selected);
            }
        });
    });

    selected.apply(move |selected| {
        let selected = selected.min(option_count.saturating_sub(1));
        with_document(|document| choice::set_selected(document, tabs, Some(selected)));
    });

    tabs
}

pub fn tabs_selected(document: &Document, tabs: NodeId) -> usize {
    let inner = document.shadow_root(tabs);
    choice::selected_index(document, inner).unwrap_or(0)
}

pub fn focus_tabs(document: &mut Document, tabs: NodeId) {
    let inner = document.shadow_root(tabs);
    choice::focus(document, inner);
}
