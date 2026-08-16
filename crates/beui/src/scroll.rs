use crate::node::NodeId;

pub(crate) struct ScrollNode {
    pub(crate) items: Vec<NodeId>,
    pub(crate) top_index: usize,
    pub(crate) offset: f32,
}

impl ScrollNode {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            top_index: 0,
            offset: 0.0,
        }
    }
}
