use egui::Color32;

use crate::node::NodeId;

pub(crate) struct FillNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) color: Color32,
    pub(crate) corner_radius: u8,
}

impl FillNode {
    pub(crate) fn new(color: Color32, corner_radius: u8) -> Self {
        Self {
            child: None,
            color,
            corner_radius,
        }
    }
}
