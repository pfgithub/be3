use egui::Color32;

use crate::node::NodeId;

pub(crate) struct OutlineNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) color: Color32,
    pub(crate) width: f32,
    pub(crate) corner_radius: u8,
    pub(crate) visible: bool,
}

impl OutlineNode {
    pub(crate) fn new(color: Color32, width: f32, corner_radius: u8) -> Self {
        Self {
            child: None,
            color,
            width,
            corner_radius,
            visible: false,
        }
    }
}
