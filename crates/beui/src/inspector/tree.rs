use std::collections::HashMap;

use accesskit::{Node as AccessNode, NodeId as AccessNodeId};

use crate::document::Document;
use crate::node::NodeId;

use super::State;

const AUTO_EXPAND_DEPTH: usize = 3;
const DETAIL_LIMIT: usize = 24;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    Node(NodeId),
    AccessKit(NodeId),
}

impl Key {
    pub(crate) fn node(self) -> NodeId {
        match self {
            Key::Node(id) | Key::AccessKit(id) => id,
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Entry {
    pub(crate) key: Key,
    pub(crate) depth: usize,
    pub(crate) kind: String,
    pub(crate) expandable: bool,
    pub(crate) expanded: bool,
    pub(crate) selected: bool,
    pub(crate) detail: String,
    pub(crate) size: String,
}

pub(crate) fn collect(target: &Document, state: &State) -> Vec<Entry> {
    let mut entries = Vec::new();
    if let Some(root) = target.root() {
        visit(target, state, Key::Node(root), 0, &mut entries);
    }
    entries
}

pub(crate) fn collect_accesskit(target: &Document, state: &State) -> Vec<Entry> {
    let Some(fragment) = target.accessibility_fragment() else {
        return Vec::new();
    };
    let nodes: HashMap<_, _> = fragment.nodes.into_iter().collect();
    let mut entries = Vec::new();
    visit_accesskit(target, state, &nodes, fragment.root, 0, &mut entries);
    entries
}

pub(crate) fn accesskit_count(target: &Document) -> usize {
    target
        .accessibility_fragment()
        .map_or(0, |fragment| fragment.nodes.len())
}

pub(crate) fn accesskit_path(target: &Document, id: NodeId) -> Vec<Key> {
    let Some(fragment) = target.accessibility_fragment() else {
        return Vec::new();
    };
    let nodes: HashMap<_, _> = fragment.nodes.into_iter().collect();
    let mut path = Vec::new();
    descend_accesskit(target, &nodes, fragment.root, id, &mut path);
    path
}

fn descend_accesskit(
    target: &Document,
    nodes: &HashMap<AccessNodeId, AccessNode>,
    access_id: AccessNodeId,
    id: NodeId,
    path: &mut Vec<Key>,
) -> bool {
    let Some(node_id) = target.local_node_id(access_id) else {
        return false;
    };
    path.push(Key::AccessKit(node_id));
    if node_id == id {
        return true;
    }
    if let Some(node) = nodes.get(&access_id) {
        for child in node.children() {
            if descend_accesskit(target, nodes, *child, id, path) {
                return true;
            }
        }
    }
    path.pop();
    false
}

fn visit_accesskit(
    target: &Document,
    state: &State,
    nodes: &HashMap<AccessNodeId, AccessNode>,
    access_id: AccessNodeId,
    depth: usize,
    entries: &mut Vec<Entry>,
) {
    let Some(node) = nodes.get(&access_id) else {
        return;
    };
    let Some(id) = target.local_node_id(access_id) else {
        return;
    };
    let key = Key::AccessKit(id);
    let expandable = !node.children().is_empty();
    let expanded = expandable && state.expanded(key, depth < AUTO_EXPAND_DEPTH);
    entries.push(Entry {
        key,
        depth,
        kind: format!("{:?}", node.role()),
        expandable,
        expanded,
        selected: state.selected.get() == Some(id),
        detail: accesskit_detail(node),
        size: accesskit_size(node),
    });
    if expanded {
        for child in node.children() {
            visit_accesskit(target, state, nodes, *child, depth + 1, entries);
        }
    }
}

fn visit(target: &Document, state: &State, key: Key, depth: usize, entries: &mut Vec<Entry>) {
    let children = children(target, key);
    let expandable = !children.is_empty();
    let expanded = expandable && state.expanded(key, auto_expand(key, depth));
    entries.push(Entry {
        key,
        depth,
        kind: kind(target, key).to_owned(),
        expandable,
        expanded,
        selected: state.selected.get() == Some(key.node()),
        detail: detail(target, key),
        size: size(target, key.node()),
    });
    if expanded {
        for child in children {
            visit(target, state, child, depth + 1, entries);
        }
    }
}

pub(crate) fn path(target: &Document, id: NodeId) -> Vec<Key> {
    let mut path = Vec::new();
    if let Some(root) = target.root() {
        descend(target, Key::Node(root), id, &mut path);
    }
    path
}

fn descend(target: &Document, key: Key, id: NodeId, path: &mut Vec<Key>) -> bool {
    path.push(key);
    if key == Key::Node(id) {
        return true;
    }
    for child in children(target, key) {
        if descend(target, child, id, path) {
            return true;
        }
    }
    path.pop();
    false
}

pub(crate) fn count(target: &Document, id: NodeId) -> usize {
    1 + target
        .children(id)
        .into_iter()
        .map(|child| count(target, child))
        .sum::<usize>()
}

pub(crate) fn label(target: &Document, id: NodeId) -> String {
    match target.node_detail(id) {
        Some(detail) => format!("{} {}", target.node_kind(id), trim(&detail)),
        None => target.node_kind(id).to_owned(),
    }
}

fn children(target: &Document, key: Key) -> Vec<Key> {
    match key {
        Key::AccessKit(_) => Vec::new(),
        Key::Node(id) => target.children(id).into_iter().map(Key::Node).collect(),
    }
}

fn kind(target: &Document, key: Key) -> &'static str {
    match key {
        Key::Node(id) => target.node_kind(id),
        Key::AccessKit(_) => unreachable!(),
    }
}

fn auto_expand(_key: Key, depth: usize) -> bool {
    depth < AUTO_EXPAND_DEPTH
}

fn detail(target: &Document, key: Key) -> String {
    let detail = match key {
        Key::Node(id) => target.node_detail(id),
        Key::AccessKit(_) => unreachable!(),
    };
    detail.map(|detail| trim(&detail)).unwrap_or_default()
}

fn accesskit_detail(node: &AccessNode) -> String {
    node.label()
        .or_else(|| node.value())
        .or_else(|| node.description())
        .map(trim)
        .unwrap_or_default()
}

fn accesskit_size(node: &AccessNode) -> String {
    match node.bounds() {
        Some(bounds) => format!("{} x {}", bounds.width().round(), bounds.height().round()),
        None => String::new(),
    }
}

fn size(target: &Document, id: NodeId) -> String {
    match target.node_rect(id) {
        Some(rect) => format!("{} x {}", rect.width().round(), rect.height().round()),
        None => String::new(),
    }
}

fn trim(detail: &str) -> String {
    let detail = detail.replace(['\n', '\t'], " ");
    if detail.chars().count() <= DETAIL_LIMIT {
        return detail;
    }
    let kept: String = detail.chars().take(DETAIL_LIMIT).collect();
    format!("{kept}...")
}
