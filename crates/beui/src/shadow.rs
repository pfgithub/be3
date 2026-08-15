use crate::node::NodeId;

pub(crate) struct ShadowNode {
    pub(crate) shadow_root: NodeId,
    pub(crate) slot: NodeId,
}

pub(crate) struct SlotNode {
    pub(crate) content: Option<NodeId>,
}
