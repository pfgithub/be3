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
         call it while building nodes or from inside event dispatch"
    );
    f(unsafe { &mut *ptr })
}

pub fn bind(document: &mut Document, mut effect: impl FnMut(&mut Document) + 'static) {
    enter(document, || {
        create_effect(move || with_document(&mut effect));
    });
}

pub trait IntoTextValue {
    fn bind_into(self, document: &mut Document, node: NodeId);
}

impl IntoTextValue for &str {
    fn bind_into(self, document: &mut Document, node: NodeId) {
        document.set_text(node, self);
    }
}

impl IntoTextValue for String {
    fn bind_into(self, document: &mut Document, node: NodeId) {
        document.set_text(node, self);
    }
}

impl<T> IntoTextValue for ReadSignal<T>
where
    T: ToString + Clone + 'static,
{
    fn bind_into(self, document: &mut Document, node: NodeId) {
        bind(document, move |document| {
            document.set_text(node, self.get().to_string());
        });
    }
}

impl<T> IntoTextValue for Memo<T>
where
    T: ToString + Clone + PartialEq + 'static,
{
    fn bind_into(self, document: &mut Document, node: NodeId) {
        bind(document, move |document| {
            document.set_text(node, self.get().to_string());
        });
    }
}

pub fn text(document: &mut Document, value: impl IntoTextValue) -> NodeId {
    let node = document.create_text(String::new(), 14.0, crate::Color32::WHITE);
    value.bind_into(document, node);
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

pub fn row(
    document: &mut Document,
    spacing: f32,
    children: impl IntoIterator<Item = (NodeId, ItemSize)>,
) -> NodeId {
    let row = unstyled::row(document, spacing);
    for (child, size) in children {
        document.append_child(row, child, size);
    }
    row
}

pub fn column(
    document: &mut Document,
    spacing: f32,
    children: impl IntoIterator<Item = (NodeId, ItemSize)>,
) -> NodeId {
    let column = unstyled::column(document, spacing);
    for (child, size) in children {
        document.append_child(column, child, size);
    }
    column
}

pub fn button(
    document: &mut Document,
    child: NodeId,
    on_click: impl FnMut(&mut Document) + 'static,
) -> NodeId {
    let button = unstyled::button(document);
    unstyled::set_button_child(document, button, child);
    unstyled::set_button_on_click(document, button, on_click);
    button
}

pub fn on_click(mut handler: impl FnMut() + 'static) -> ClickHandler {
    Box::new(move |_document| handler())
}
