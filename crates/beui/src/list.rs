use crate::node::NodeId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ItemSize {
    Intrinsic,
    Percent(f32),
}

pub(crate) struct ListItem {
    pub(crate) child: NodeId,
    pub(crate) size: ItemSize,
}

pub(crate) struct ListNode {
    pub(crate) direction: Direction,
    pub(crate) items: Vec<ListItem>,
}
