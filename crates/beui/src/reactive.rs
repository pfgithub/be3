use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use reactive::create_effect;

use crate::base::ItemSize;
use crate::document::Document;
use crate::node::{ClickHandler, NodeId};
use crate::unstyled;

pub use beui_macros::{component, view};
pub use reactive::{
    batch, create_memo, create_signal, on_cleanup, untrack, Memo, ReadSignal, Scope, WriteSignal,
};

thread_local! {
    static CURRENT_DOCUMENT: RefCell<Option<Document>> = const { RefCell::new(None) };
    static CURRENT_COMPONENT: Cell<Option<NodeId>> = const { Cell::new(None) };
    static PENDING_DETAIL: RefCell<HashMap<NodeId, String>> = RefCell::new(HashMap::new());
}

pub(crate) enum DocumentGuard<'a> {
    Installed(&'a mut Document),
    Reentrant,
}

impl Drop for DocumentGuard<'_> {
    fn drop(&mut self) {
        if let DocumentGuard::Installed(document) = self {
            let restored = CURRENT_DOCUMENT.with(|cell| cell.borrow_mut().take());
            **document = restored
                .expect("beui::reactive document guard dropped without an installed document");
        }
    }
}

pub(crate) fn install(document: &mut Document) -> DocumentGuard<'_> {
    let already_installed = CURRENT_DOCUMENT
        .with(|cell| cell.try_borrow().map(|slot| slot.is_some()))
        .unwrap_or(true);
    if already_installed {
        return DocumentGuard::Reentrant;
    }
    let taken = std::mem::take(document);
    CURRENT_DOCUMENT.with(|cell| {
        let previous = cell.borrow_mut().replace(taken);
        assert!(
            previous.is_none(),
            "beui::reactive: a document is already installed on this thread"
        );
    });
    DocumentGuard::Installed(document)
}

pub(crate) fn enter<R>(document: &mut Document, f: impl FnOnce() -> R) -> R {
    let context = document.reactive_scope().context();
    let _guard = install(document);
    context.run(f)
}

pub fn with_reactive_scope<R>(document: &mut Document, f: impl FnOnce() -> R) -> R {
    enter(document, f)
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
    let shadow = with_document(Document::reserve_shadow);
    let previous = CURRENT_COMPONENT.with(|cell| cell.replace(Some(shadow)));
    let root = f();
    CURRENT_COMPONENT.with(|cell| cell.set(previous));
    with_document(|document| document.finish_shadow(shadow, name, root, Vec::new()));
    if let Some(detail) = PENDING_DETAIL.with(|cell| cell.borrow_mut().remove(&shadow)) {
        with_document(|document| document.set_component_detail(shadow, detail));
    }
    shadow
}

pub fn current_component() -> NodeId {
    CURRENT_COMPONENT
        .with(Cell::get)
        .expect("current_component() called outside of a #[component] body")
}

pub fn set_component_detail(document: &mut Document, shadow: NodeId, detail: impl Into<String>) {
    let detail = detail.into();
    if document.contains(shadow) {
        document.set_component_detail(shadow, detail);
    } else {
        PENDING_DETAIL.with(|cell| {
            cell.borrow_mut().insert(shadow, detail);
        });
    }
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

impl<T: 'static> IntoProp<T> for Prop<T> {
    fn into_prop(self) -> Prop<T> {
        self
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

#[component]
pub fn text(string: Prop<String>) -> NodeId {
    let node =
        with_document(|document| document.create_text(String::new(), 14.0, crate::Color32::WHITE));
    string.apply(move |value| with_document(|document| document.set_text(node, value)));
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

pub struct Children(Vec<(NodeId, ItemSize)>);

impl Children {
    fn mount(self, parent: NodeId) {
        with_document(|document| {
            for (child, size) in self.0 {
                document.append_child(parent, child, size);
            }
        });
    }

    pub(crate) fn into_first(self) -> Option<NodeId> {
        self.0.into_iter().next().map(|(child, _)| child)
    }
}

impl<I: IntoIterator<Item = (NodeId, ItemSize)>> From<I> for Children {
    fn from(children: I) -> Self {
        Children(children.into_iter().collect())
    }
}

#[component]
pub fn row(spacing: f32, children: Children) -> NodeId {
    let row = with_document(|document| unstyled::row(document, spacing));
    children.mount(row);
    row
}

#[component]
pub fn column(spacing: f32, children: Children) -> NodeId {
    let column = with_document(|document| unstyled::column(document, spacing));
    children.mount(column);
    column
}

#[component]
pub fn show(condition: Prop<bool>, then: Option<Box<dyn FnOnce() -> NodeId>>) -> NodeId {
    let mut then = then;
    let visibility = with_document(|document| document.create_visibility(false));
    let built: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
    condition.apply(move |visible| {
        if visible && built.get().is_none() {
            let child = then.take().expect("show requires a `then` callback")();
            built.set(Some(child));
            with_document(|document| document.set_visibility_child(visibility, child));
        }
        with_document(|document| document.set_visible(visibility, visible));
    });
    visibility
}

type ForEachView<T> = Box<dyn Fn(&T) -> (NodeId, ItemSize)>;

#[component]
pub fn for_each<T, K>(
    spacing: f32,
    items: Prop<Vec<T>>,
    key: Option<Box<dyn Fn(&T) -> K>>,
    view: Option<ForEachView<T>>,
) -> NodeId
where
    T: 'static,
    K: Hash + Eq + 'static,
{
    let key = key.expect("for_each requires a `key` callback");
    let view = view.expect("for_each requires a `view` callback");
    let parent = with_document(|document| unstyled::column(document, spacing));
    let existing: Rc<RefCell<HashMap<K, (NodeId, ItemSize)>>> =
        Rc::new(RefCell::new(HashMap::new()));
    items.apply(move |items| {
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
    parent
}

#[component]
pub fn button(children: Children, disabled: Prop<bool>, on_click: Option<ClickHandler>) -> NodeId {
    let button = with_document(unstyled::button);
    if let Some(child) = children.into_first() {
        with_document(|document| unstyled::set_button_child(document, button, child));
    }
    if let Some(on_click) = on_click {
        with_document(|document| unstyled::set_button_on_click(document, button, on_click));
    }
    disabled.apply(move |disabled| {
        with_document(|document| unstyled::set_button_disabled(document, button, disabled))
    });
    button
}
