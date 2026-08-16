use crate::node::NodeId;

pub(crate) struct PaddingNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) amount: f32,
}

impl PaddingNode {
    pub(crate) fn new(amount: f32) -> Self {
        Self {
            child: None,
            amount,
        }
    }
}
