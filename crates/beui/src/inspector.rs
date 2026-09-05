use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use egui::{Context, Rect};

use crate::base::{ItemSize, TextAlign};
use crate::document::Document;
use crate::node::NodeId;
use crate::styled;
use crate::styled::theme::{SCROLLBAR_WIDTH, SEPARATOR_HEIGHT, SURFACE, TEXT_MUTED};
use crate::unstyled;

pub(crate) const PANEL_WIDTH: f32 = 320.0;

const HEADER_PADDING: f32 = 12.0;
const HEADER_SPACING: f32 = 8.0;
const BODY_PADDING: f32 = 8.0;
const BODY_SPACING: f32 = 6.0;
const ROW_SPACING: f32 = 6.0;
const INDENT: f32 = 12.0;
const MARKER_WIDTH: f32 = 8.0;
const AUTO_EXPAND_DEPTH: usize = 3;
const DETAIL_LIMIT: usize = 24;

type Expansion = Rc<RefCell<HashMap<NodeId, bool>>>;

pub(crate) struct Entry {
    pub(crate) id: NodeId,
    pub(crate) depth: usize,
    pub(crate) kind: &'static str,
    expandable: bool,
    expanded: bool,
    detail: String,
    size: String,
}

impl Entry {
    fn same_shape(&self, other: &Entry) -> bool {
        self.id == other.id
            && self.depth == other.depth
            && self.expandable == other.expandable
            && self.expanded == other.expanded
    }
}

pub(crate) struct Row {
    pub(crate) row: NodeId,
    detail: NodeId,
    size: NodeId,
}

struct Panel {
    document: Document,
    scroll: NodeId,
    count: NodeId,
    rows: Vec<Row>,
}

pub(crate) struct Inspector {
    pub(crate) document: Document,
    pub(crate) entries: Vec<Entry>,
    pub(crate) rows: Vec<Row>,
    scroll: NodeId,
    count: NodeId,
    total: usize,
    offset: f32,
    expansion: Expansion,
    revision: Rc<Cell<u64>>,
    seen: u64,
}

impl Inspector {
    pub(crate) fn new() -> Self {
        let expansion: Expansion = Rc::new(RefCell::new(HashMap::new()));
        let revision = Rc::new(Cell::new(0));
        let panel = build(&[], 0, &expansion, &revision, 0.0);
        Self {
            document: panel.document,
            entries: Vec::new(),
            rows: panel.rows,
            scroll: panel.scroll,
            count: panel.count,
            total: 0,
            offset: 0.0,
            expansion,
            revision,
            seen: 0,
        }
    }

    pub(crate) fn show(&mut self, target: &Document, ctx: &Context, rect: Rect) {
        self.sync(target);
        self.document.show(ctx, rect);
        self.offset = self.document.scroll_offset(self.scroll);
        if self.revision.get() != self.seen {
            self.seen = self.revision.get();
            ctx.request_repaint();
        }
    }

    fn sync(&mut self, target: &Document) {
        let entries = collect(target, &self.expansion.borrow());
        let total = target.root().map_or(0, |root| count(target, root));
        if self.reshaped(&entries) {
            let panel = build(
                &entries,
                total,
                &self.expansion,
                &self.revision,
                self.offset,
            );
            self.document = panel.document;
            self.scroll = panel.scroll;
            self.count = panel.count;
            self.rows = panel.rows;
            self.entries = entries;
            self.total = total;
            return;
        }

        self.update(entries);
        if total != self.total {
            self.total = total;
            self.document.set_text(self.count, total_label(total));
        }
    }

    fn reshaped(&self, entries: &[Entry]) -> bool {
        entries.len() != self.entries.len()
            || entries
                .iter()
                .zip(&self.entries)
                .any(|(entry, previous)| !entry.same_shape(previous))
    }

    fn update(&mut self, entries: Vec<Entry>) {
        for (index, entry) in entries.into_iter().enumerate() {
            let previous = &mut self.entries[index];
            let row = &self.rows[index];
            if entry.detail != previous.detail {
                self.document.set_text(row.detail, entry.detail.clone());
            }
            if entry.size != previous.size {
                self.document.set_text(row.size, entry.size.clone());
            }
            *previous = entry;
        }
    }
}

fn collect(target: &Document, expansion: &HashMap<NodeId, bool>) -> Vec<Entry> {
    let mut entries = Vec::new();
    if let Some(root) = target.root() {
        visit(target, expansion, root, 0, &mut entries);
    }
    entries
}

fn visit(
    target: &Document,
    expansion: &HashMap<NodeId, bool>,
    id: NodeId,
    depth: usize,
    entries: &mut Vec<Entry>,
) {
    let children = target.children(id);
    let expandable = !children.is_empty();
    let expanded = expandable && is_expanded(expansion, id, depth);
    entries.push(Entry {
        id,
        depth,
        kind: target.node_kind(id),
        expandable,
        expanded,
        detail: detail(target, id),
        size: size(target, id),
    });
    if expanded {
        for child in children {
            visit(target, expansion, child, depth + 1, entries);
        }
    }
}

fn is_expanded(expansion: &HashMap<NodeId, bool>, id: NodeId, depth: usize) -> bool {
    expansion
        .get(&id)
        .copied()
        .unwrap_or(depth < AUTO_EXPAND_DEPTH)
}

fn count(target: &Document, id: NodeId) -> usize {
    1 + target
        .children(id)
        .into_iter()
        .map(|child| count(target, child))
        .sum::<usize>()
}

fn detail(target: &Document, id: NodeId) -> String {
    let Some(detail) = target.node_detail(id) else {
        return String::new();
    };
    let detail = detail.replace(['\n', '\t'], " ");
    if detail.chars().count() <= DETAIL_LIMIT {
        return detail;
    }
    let kept: String = detail.chars().take(DETAIL_LIMIT).collect();
    format!("{kept}...")
}

fn size(target: &Document, id: NodeId) -> String {
    match target.node_rect(id) {
        Some(rect) => format!("{} x {}", rect.width().round(), rect.height().round()),
        None => String::new(),
    }
}

fn build(
    entries: &[Entry],
    total: usize,
    expansion: &Expansion,
    revision: &Rc<Cell<u64>>,
    offset: f32,
) -> Panel {
    let mut document = Document::new();
    document.inspectable = false;

    let count = header_count(&mut document, total);
    let header = header(&mut document, count);

    let scroll = document.create_scroll();
    let mut rows = Vec::new();
    for entry in entries {
        let row = row(&mut document, entry, expansion, revision);
        document.append_scroll_item(scroll, row.row);
        rows.push(row);
    }
    document.set_scroll_offset(scroll, offset);
    let body = body(&mut document, scroll);

    let column = unstyled::column(&mut document, 0.0);
    document.append_child(column, header, ItemSize::Intrinsic);
    let line = styled::separator(&mut document);
    document.append_child(column, line, ItemSize::Fixed(SEPARATOR_HEIGHT));
    document.append_child(column, body, ItemSize::Percent(100.0));

    let surface = document.create_fill(SURFACE, 0);
    document.set_fill_child(surface, column);

    let edge = styled::separator(&mut document);
    let panel = unstyled::row(&mut document, 0.0);
    document.append_child(panel, edge, ItemSize::Fixed(SEPARATOR_HEIGHT));
    document.append_child(panel, surface, ItemSize::Percent(100.0));
    document.set_root(panel);

    Panel {
        document,
        scroll,
        count,
        rows,
    }
}

fn header_count(document: &mut Document, total: usize) -> NodeId {
    let count = styled::caption(document, total_label(total));
    document.set_text_align(count, TextAlign::End, TextAlign::Center);
    count
}

fn total_label(total: usize) -> String {
    match total {
        1 => "1 node".to_owned(),
        total => format!("{total} nodes"),
    }
}

fn header(document: &mut Document, count: NodeId) -> NodeId {
    let title = styled::heading(document, "Inspector");
    let line = unstyled::centered_row(document, HEADER_SPACING);
    document.append_child(line, title, ItemSize::Intrinsic);
    document.append_child(line, count, ItemSize::Percent(100.0));
    let padding = document.create_padding(HEADER_PADDING, HEADER_PADDING);
    document.set_padding_child(padding, line);
    padding
}

fn body(document: &mut Document, scroll: NodeId) -> NodeId {
    let bar = styled::scrollbar(document, scroll);
    let area = unstyled::row(document, BODY_SPACING);
    document.append_child(area, scroll, ItemSize::Percent(100.0));
    document.append_child(area, bar, ItemSize::Fixed(SCROLLBAR_WIDTH));
    let padding = document.create_padding(BODY_PADDING, BODY_PADDING);
    document.set_padding_child(padding, area);
    padding
}

fn row(
    document: &mut Document,
    entry: &Entry,
    expansion: &Expansion,
    revision: &Rc<Cell<u64>>,
) -> Row {
    let indent = unstyled::spacer(document);

    let marker = styled::code(document, marker(entry));
    document.set_text_color(marker, TEXT_MUTED);

    let kind = styled::code(document, entry.kind);

    let detail = styled::code(document, entry.detail.clone());
    document.set_text_color(detail, TEXT_MUTED);

    let size = styled::code(document, entry.size.clone());
    document.set_text_color(size, TEXT_MUTED);
    document.set_text_align(size, TextAlign::End, TextAlign::Center);

    let line = unstyled::centered_row(document, ROW_SPACING);
    document.append_child(line, indent, ItemSize::Fixed(entry.depth as f32 * INDENT));
    document.append_child(line, marker, ItemSize::Fixed(MARKER_WIDTH));
    document.append_child(line, kind, ItemSize::Intrinsic);
    document.append_child(line, detail, ItemSize::Percent(100.0));
    document.append_child(line, size, ItemSize::Intrinsic);

    let row = styled::list_row(document, line);
    if entry.expandable {
        let expansion = expansion.clone();
        let revision = revision.clone();
        let id = entry.id;
        let expanded = entry.expanded;
        unstyled::set_pressable_on_click(document, row, move |_document| {
            expansion.borrow_mut().insert(id, !expanded);
            revision.set(revision.get() + 1);
        });
    }

    Row { row, detail, size }
}

fn marker(entry: &Entry) -> &'static str {
    match (entry.expandable, entry.expanded) {
        (true, true) => "-",
        (true, false) => "+",
        (false, _) => "",
    }
}
