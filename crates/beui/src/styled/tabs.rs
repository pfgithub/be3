use super::choice::{self, Kind};
use crate::document::Document;
use crate::node::NodeId;

pub fn tabs(document: &mut Document, labels: &[&str], selected: usize) -> NodeId {
    choice::choice(
        document,
        labels,
        Some(selected.min(labels.len().saturating_sub(1))),
        Kind::Tabs,
    )
}

pub fn tabs_selected(document: &Document, tabs: NodeId) -> usize {
    choice::selected_index(document, tabs).unwrap_or(0)
}

pub fn set_tabs_selected(document: &mut Document, tabs: NodeId, selected: usize) {
    choice::set_selected(document, tabs, Some(selected));
}

pub fn set_tabs_on_change(
    document: &mut Document,
    tabs: NodeId,
    mut handler: impl FnMut(&mut Document, usize) + 'static,
) {
    choice::set_on_change(document, tabs, move |document, selected| {
        if let Some(selected) = selected {
            handler(document, selected);
        }
    });
}

pub fn focus_tabs(document: &mut Document, tabs: NodeId) {
    choice::focus(document, tabs);
}
