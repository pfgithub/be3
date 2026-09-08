use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use reactive::{create_effect, ReadSignal};

use crate::base::ItemSize;
use crate::document::Document;
use crate::node::{ClickHandler, NodeId};
use crate::unstyled;

pub use beui_macros::{builder, component, view};
pub use reactive::{batch, create_memo, create_signal, on_cleanup, untrack, Memo, Scope};

thread_local! {
    static CURRENT_DOCUMENT: RefCell<Option<Document>> = const { RefCell::new(None) };
}

pub(crate) struct DocumentGuard<'a> {
    document: &'a mut Document,
}

impl Drop for DocumentGuard<'_> {
    fn drop(&mut self) {
        let restored = CURRENT_DOCUMENT.with(|cell| cell.borrow_mut().take());
        *self.document =
            restored.expect("beui::reactive document guard dropped without an installed document");
    }
}

pub(crate) fn install(document: &mut Document) -> DocumentGuard<'_> {
    let taken = std::mem::take(document);
    CURRENT_DOCUMENT.with(|cell| {
        let previous = cell.borrow_mut().replace(taken);
        assert!(
            previous.is_none(),
            "beui::reactive: a document is already installed on this thread"
        );
    });
    DocumentGuard { document }
}

pub(crate) fn enter<R>(document: &mut Document, f: impl FnOnce() -> R) -> R {
    let context = document.reactive_scope().context();
    let _guard = install(document);
    context.run(f)
}

pub fn with_document<R>(f: impl FnOnce(&mut Document) -> R) -> R {
    CURRENT_DOCUMENT.with(|cell| {
        let mut slot = cell.borrow_mut();
        let document = slot.as_mut().expect(
            "beui::reactive binding used without an active document; \
             call it from inside build(), or from inside event dispatch",
        );
        f(document)
    })
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

pub fn component(name: &'static str, f: impl FnOnce() -> NodeId) -> NodeId {
    let root = f();
    with_document(|document| document.create_shadow(name, root, Vec::new()))
}

pub enum Prop<T> {
    Static(T),
    Dynamic(Box<dyn Fn() -> T>),
}

impl<T: 'static> Prop<T> {
    pub fn apply(self, mut set: impl FnMut(T) + 'static) {
        match self {
            Prop::Static(value) => set(value),
            Prop::Dynamic(read) => {
                create_effect(move || set(read()));
            }
        }
    }

    pub fn map<U: 'static>(self, f: impl Fn(T) -> U + 'static) -> Prop<U> {
        match self {
            Prop::Static(value) => Prop::Static(f(value)),
            Prop::Dynamic(read) => Prop::Dynamic(Box::new(move || f(read()))),
        }
    }
}

pub trait IntoProp<T> {
    fn into_prop(self) -> Prop<T>;
}

impl<T: 'static> IntoProp<T> for T {
    fn into_prop(self) -> Prop<T> {
        Prop::Static(self)
    }
}

impl<T: Clone + 'static> IntoProp<T> for ReadSignal<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(Box::new(move || self.get()))
    }
}

impl<T: Clone + PartialEq + 'static> IntoProp<T> for Memo<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(Box::new(move || self.get()))
    }
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

impl<T: ToString + Clone + 'static> IntoTextValue for ReadSignal<T> {
    fn bind_into(self, node: NodeId) {
        Prop::Dynamic(Box::new(move || self.get()))
            .map(|value| value.to_string())
            .apply(move |value| with_document(|document| document.set_text(node, value)));
    }
}

impl<T: ToString + Clone + PartialEq + 'static> IntoTextValue for Memo<T> {
    fn bind_into(self, node: NodeId) {
        Prop::Dynamic(Box::new(move || self.get()))
            .map(|value| value.to_string())
            .apply(move |value| with_document(|document| document.set_text(node, value)));
    }
}

fn boxed_text_binder(value: impl IntoTextValue + 'static) -> Box<dyn FnOnce(NodeId)> {
    Box::new(move |node| value.bind_into(node))
}

#[component]
pub fn text(
    #[prop(with = |value: impl IntoTextValue + 'static| boxed_text_binder(value))] string: Option<
        Box<dyn FnOnce(NodeId)>,
    >,
) -> NodeId {
    let node =
        with_document(|document| document.create_text(String::new(), 14.0, crate::Color32::WHITE));
    if let Some(bind) = string {
        bind(node);
    }
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

pub struct Children(Box<dyn FnOnce(NodeId)>);

impl Children {
    fn mount(self, parent: NodeId) {
        (self.0)(parent)
    }
}

impl<I: IntoIterator<Item = (NodeId, ItemSize)>> From<I> for Children {
    fn from(children: I) -> Self {
        let children: Vec<_> = children.into_iter().collect();
        Children(Box::new(move |parent| {
            with_document(|document| {
                for (child, size) in children {
                    document.append_child(parent, child, size);
                }
            });
        }))
    }
}

#[builder]
pub fn row(
    spacing: f32,
    #[prop(with = |children: impl Into<Children>| children.into())] children: Children,
) -> NodeId {
    let row = with_document(|document| unstyled::row(document, spacing));
    children.mount(row);
    row
}

#[builder]
pub fn column(
    spacing: f32,
    #[prop(with = |children: impl Into<Children>| children.into())] children: Children,
) -> NodeId {
    let column = with_document(|document| unstyled::column(document, spacing));
    children.mount(column);
    column
}

fn boxed_show_then(then: impl Fn() -> NodeId + 'static) -> Box<dyn Fn() -> NodeId> {
    Box::new(then)
}

#[component]
pub fn show(
    condition: Prop<bool>,
    #[prop(with = |then: impl Fn() -> NodeId + 'static| boxed_show_then(then))] then: Option<
        Box<dyn Fn() -> NodeId>,
    >,
) -> NodeId {
    let then = then.expect("show requires a `then` callback");
    let visibility = with_document(|document| document.create_visibility(false));
    let built: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
    condition.apply(move |visible| {
        if visible && built.get().is_none() {
            let child = then();
            built.set(Some(child));
            with_document(|document| document.set_visibility_child(visibility, child));
        }
        with_document(|document| document.set_visible(visibility, visible));
    });
    visibility
}

pub fn for_each<T, K>(
    items: impl IntoProp<Vec<T>> + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(&T) -> (NodeId, ItemSize) + 'static,
) -> Children
where
    T: 'static,
    K: Hash + Eq + 'static,
{
    Children(Box::new(move |parent| {
        let existing: Rc<RefCell<HashMap<K, (NodeId, ItemSize)>>> =
            Rc::new(RefCell::new(HashMap::new()));
        items.into_prop().apply(move |items| {
            let mut existing = existing.borrow_mut();
            let mut next = Vec::with_capacity(items.len());
            for item in &items {
                let entry = existing.remove(&key(item)).unwrap_or_else(|| view(item));
                next.push((key(item), entry));
            }
            with_document(|document| {
                for (removed, _) in existing.values() {
                    document.remove_child(parent, *removed);
                    document.remove_node(*removed);
                }
                for (_, (child, _)) in &next {
                    document.remove_child(parent, *child);
                }
                for (_, (child, size)) in &next {
                    document.append_child(parent, *child, *size);
                }
            });
            existing.clear();
            existing.extend(next);
        });
    }))
}

fn boxed_click_handler(mut handler: impl FnMut() + 'static) -> ClickHandler {
    Box::new(move |_document| handler())
}

fn first_child(children: impl IntoIterator<Item = (NodeId, ItemSize)>) -> NodeId {
    children
        .into_iter()
        .next()
        .expect("button requires a child")
        .0
}

#[component]
pub fn button(
    #[prop(with = |children: impl IntoIterator<Item = (NodeId, ItemSize)>| first_child(children))]
    children: Option<NodeId>,
    disabled: Prop<bool>,
    #[prop(with = |handler: impl FnMut() + 'static| boxed_click_handler(handler))] on_click: Option<
        ClickHandler,
    >,
) -> NodeId {
    let button = with_document(unstyled::button);
    if let Some(children) = children {
        with_document(|document| unstyled::set_button_child(document, button, children));
    }
    if let Some(on_click) = on_click {
        with_document(|document| unstyled::set_button_on_click(document, button, on_click));
    }
    disabled.apply(move |disabled| {
        with_document(|document| unstyled::set_button_disabled(document, button, disabled))
    });
    button
}
