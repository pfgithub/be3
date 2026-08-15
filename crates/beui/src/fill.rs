use egui::Color32;

use crate::node::NodeId;

pub(crate) struct FillNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) color: Color32,
}

impl FillNode {
    pub(crate) fn new(color: Color32) -> Self {
        Self { child: None, color }
    }
}
