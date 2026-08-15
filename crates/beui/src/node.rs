use crate::button::ButtonNode;
use crate::fill::FillNode;
use crate::list::ListNode;
use crate::outline::OutlineNode;
use crate::text::TextNode;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(u32);

pub(crate) struct Node {
    pub(crate) kind: NodeKind,
}

pub(crate) enum NodeKind {
    List(ListNode),
    Button(ButtonNode),
    Fill(FillNode),
    Outline(OutlineNode),
    Text(TextNode),
}

#[derive(Default)]
pub(crate) struct Arena {
    nodes: Vec<Option<Node>>,
}

impl Arena {
    pub(crate) fn insert(&mut self, kind: NodeKind) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Some(Node { kind }));
        id
    }

    pub(crate) fn get(&self, id: NodeId) -> &Node {
        self.nodes[id.0 as usize]
            .as_ref()
            .expect("node was removed")
    }

    pub(crate) fn get_mut(&mut self, id: NodeId) -> &mut Node {
        self.nodes[id.0 as usize]
            .as_mut()
            .expect("node was removed")
    }

    pub(crate) fn remove(&mut self, id: NodeId) {
        self.nodes[id.0 as usize] = None;
    }
}
