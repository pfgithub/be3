use std::cell::Cell;

use reactive::{create_effect, ReadSignal};

use crate::base::ItemSize;
use crate::document::Document;
use crate::node::{ClickHandler, NodeId};
use crate::unstyled;

pub use reactive::{batch, create_memo, create_signal, on_cleanup, untrack, Memo, Scope};

thread_local! {
    static CURRENT_DOCUMENT: Cell<*mut Document> = const { Cell::new(std::ptr::null_mut()) };
}

pub(crate) struct DocumentGuard {
    previous: *mut Document,
}

impl Drop for DocumentGuard {
    fn drop(&mut self) {
        CURRENT_DOCUMENT.with(|cell| cell.set(self.previous));
    }
}

pub(crate) fn install(document: &mut Document) -> DocumentGuard {
    let ptr: *mut Document = document;
    let previous = CURRENT_DOCUMENT.with(|cell| cell.replace(ptr));
    DocumentGuard { previous }
}

pub(crate) fn enter<R>(document: &mut Document, f: impl FnOnce() -> R) -> R {
    let _guard = install(document);
    document.reactive_scope().run(f)
}

pub fn with_document<R>(f: impl FnOnce(&mut Document) -> R) -> R {
    let ptr = CURRENT_DOCUMENT.with(Cell::get);
    assert!(
        !ptr.is_null(),
        "beui::reactive binding used without an active document; \
         call it from inside build(), or from inside event dispatch"
    );
    f(unsafe { &mut *ptr })
}

pub fn build(f: impl FnOnce() -> NodeId) -> Document {
    let mut document = Document::new();
    let root = enter(&mut document, f);
    document.set_root(root);
    document
}

pub fn bind(mut effect: impl FnMut(&mut Document) + 'static) {
    create_effect(move || with_document(&mut effect));
}

pub trait IntoTextValue {
    fn bind_into(self, node: NodeId);
}

impl IntoTextValue for &str {
    fn bind_into(self, node: NodeId) {
        with_document(|document| document.set_text(node, self));
    }
}

impl IntoTextValue for String {
    fn bind_into(self, node: NodeId) {
        with_document(|document| document.set_text(node, self));
    }
}

impl<T> IntoTextValue for ReadSignal<T>
where
    T: ToString + Clone + 'static,
{
    fn bind_into(self, node: NodeId) {
        bind(move |document| document.set_text(node, self.get().to_string()));
    }
}

impl<T> IntoTextValue for Memo<T>
where
    T: ToString + Clone + PartialEq + 'static,
{
    fn bind_into(self, node: NodeId) {
        bind(move |document| document.set_text(node, self.get().to_string()));
    }
}

pub fn text(value: impl IntoTextValue) -> NodeId {
    let node =
        with_document(|document| document.create_text(String::new(), 14.0, crate::Color32::WHITE));
    value.bind_into(node);
    node
}

pub fn intrinsic(node: NodeId) -> (NodeId, ItemSize) {
    (node, ItemSize::Intrinsic)
}

pub fn fixed(node: NodeId, size: f32) -> (NodeId, ItemSize) {
    (node, ItemSize::Fixed(size))
}

pub fn percent(node: NodeId, weight: f32) -> (NodeId, ItemSize) {
    (node, ItemSize::Percent(weight))
}

pub fn row(spacing: f32, children: impl IntoIterator<Item = (NodeId, ItemSize)>) -> NodeId {
    with_document(|document| {
        let row = unstyled::row(document, spacing);
        for (child, size) in children {
            document.append_child(row, child, size);
        }
        row
    })
}

pub fn column(spacing: f32, children: impl IntoIterator<Item = (NodeId, ItemSize)>) -> NodeId {
    with_document(|document| {
        let column = unstyled::column(document, spacing);
        for (child, size) in children {
            document.append_child(column, child, size);
        }
        column
    })
}

pub fn button(child: NodeId, on_click: impl FnMut(&mut Document) + 'static) -> NodeId {
    with_document(|document| {
        let button = unstyled::button(document);
        unstyled::set_button_child(document, button, child);
        unstyled::set_button_on_click(document, button, on_click);
        button
    })
}

pub fn on_click(mut handler: impl FnMut() + 'static) -> ClickHandler {
    Box::new(move |_document| handler())
}
