use crate::node::NodeId;

pub(crate) struct ButtonNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) armed: bool,
    pub(crate) clicked: bool,
}

impl ButtonNode {
    pub(crate) fn new() -> Self {
        Self {
            child: None,
            armed: false,
            clicked: false,
        }
    }
}
