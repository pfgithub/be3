use super::choice::{self, Kind};
use crate::document::Document;
use crate::node::NodeId;

pub fn radio_group(document: &mut Document, labels: &[&str], selected: Option<usize>) -> NodeId {
    choice::choice(document, labels, selected, Kind::Radio)
}

pub fn radio_group_selected(document: &Document, control: NodeId) -> Option<usize> {
    choice::selected_index(document, control)
}

pub fn set_radio_group_selected(document: &mut Document, control: NodeId, selected: Option<usize>) {
    choice::set_selected(document, control, selected);
}

pub fn set_radio_group_on_change(
    document: &mut Document,
    control: NodeId,
    handler: impl FnMut(&mut Document, Option<usize>) + 'static,
) {
    choice::set_on_change(document, control, handler);
}

pub fn focus_radio_group(document: &mut Document, control: NodeId) {
    choice::focus(document, control);
}
