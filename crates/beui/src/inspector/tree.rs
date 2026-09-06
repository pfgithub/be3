use crate::document::Document;
use crate::node::NodeId;

use super::State;

const AUTO_EXPAND_DEPTH: usize = 3;
const DETAIL_LIMIT: usize = 24;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    Node(NodeId),
    Internals(NodeId),
    Placeholder(NodeId),
}

impl Key {
    pub(crate) fn node(self) -> NodeId {
        match self {
            Key::Node(id) | Key::Internals(id) | Key::Placeholder(id) => id,
        }
    }
}

pub(crate) struct Entry {
    pub(crate) key: Key,
    pub(crate) depth: usize,
    pub(crate) kind: &'static str,
    pub(crate) expandable: bool,
    pub(crate) expanded: bool,
    pub(crate) selected: bool,
    pub(crate) detail: String,
    pub(crate) size: String,
}

impl Entry {
    pub(crate) fn same_shape(&self, other: &Entry) -> bool {
        self.key == other.key
            && self.depth == other.depth
            && self.kind == other.kind
            && self.expandable == other.expandable
            && self.expanded == other.expanded
    }
}

pub(crate) fn collect(target: &Document, state: &State) -> Vec<Entry> {
    let mut entries = Vec::new();
    if let Some(root) = target.root() {
        visit(target, state, Key::Node(root), 0, &mut entries);
    }
    entries
}

fn visit(target: &Document, state: &State, key: Key, depth: usize, entries: &mut Vec<Entry>) {
    let children = children(target, key);
    let expandable = !children.is_empty();
    let expanded = expandable && state.expanded(key, auto_expand(key, depth));
    entries.push(Entry {
        key,
        depth,
        kind: kind(target, key),
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
        Key::Placeholder(_) => Vec::new(),
        Key::Internals(shadow) => vec![child_key(target, target.shadow_root(shadow))],
        Key::Node(id) if target.as_shadow(id).is_some() => std::iter::once(Key::Internals(id))
            .chain(target.shadow_slots(id).into_iter().map(Key::Node))
            .collect(),
        Key::Node(id) => target
            .children(id)
            .into_iter()
            .map(|child| child_key(target, child))
            .collect(),
    }
}

fn child_key(target: &Document, id: NodeId) -> Key {
    match target.as_slot(id) {
        Some(_) => Key::Placeholder(id),
        None => Key::Node(id),
    }
}

fn kind(target: &Document, key: Key) -> &'static str {
    match key {
        Key::Node(id) => target.node_kind(id),
        Key::Internals(_) => "shadow",
        Key::Placeholder(_) => "slot",
    }
}

fn auto_expand(key: Key, depth: usize) -> bool {
    !matches!(key, Key::Internals(_)) && depth < AUTO_EXPAND_DEPTH
}

fn detail(target: &Document, key: Key) -> String {
    let detail = match key {
        Key::Node(id) => target.node_detail(id),
        Key::Internals(_) => None,
        Key::Placeholder(id) => Some(target.node_kind(id).to_owned()),
    };
    detail.map(|detail| trim(&detail)).unwrap_or_default()
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
