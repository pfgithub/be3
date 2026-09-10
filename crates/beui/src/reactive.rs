use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use crate::base::ItemSize;
use crate::color::Color32;
use crate::document::Document;
use crate::node::{ClickHandler, NodeId};
use crate::unstyled;

pub use beui_macros::{component, view};
pub use reactive::{
    batch, create_effect, create_memo, create_signal, on_cleanup, untrack, Effect, Memo,
    ReadSignal, Scope, WriteSignal,
};

thread_local! {
    static CURRENT_DOCUMENT: RefCell<Option<Document>> = const { RefCell::new(None) };
    static ACTIVE_DOCUMENT: Cell<*mut Document> = const { Cell::new(std::ptr::null_mut()) };
    static CURRENT_COMPONENT: Cell<Option<NodeId>> = const { Cell::new(None) };
    static PENDING_DETAIL: RefCell<HashMap<NodeId, String>> = RefCell::new(HashMap::new());
    static PENDING_STATE: RefCell<HashMap<NodeId, Box<dyn Any>>> = RefCell::new(HashMap::new());
}

struct ActiveDocumentGuard;

impl Drop for ActiveDocumentGuard {
    fn drop(&mut self) {
        ACTIVE_DOCUMENT.with(|active| active.set(std::ptr::null_mut()));
    }
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
    CURRENT_DOCUMENT.with(|cell| match cell.try_borrow_mut() {
        Ok(mut slot) => {
            let document = slot.as_mut().expect(
                "beui::reactive binding used without an active document; \
                 call it from inside build(), or from inside event dispatch",
            );
            let ptr: *mut Document = document;
            ACTIVE_DOCUMENT.with(|active| active.set(ptr));
            let _guard = ActiveDocumentGuard;
            f(document)
        }
        Err(_) => {
            let ptr = ACTIVE_DOCUMENT.with(Cell::get);
            assert!(
                !ptr.is_null(),
                "beui::reactive binding used without an active document; \
                 call it from inside build(), or from inside event dispatch"
            );
            f(unsafe { &mut *ptr })
        }
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

pub fn in_new_scope(f: impl FnOnce() -> NodeId) -> NodeId {
    let scope = with_document(|document| document.reactive_scope().context().run(Scope::new));
    let node = scope.context().run(f);
    with_document(|document| document.register_node_scope(node, scope));
    node
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
    if let Some(state) = PENDING_STATE.with(|cell| cell.borrow_mut().remove(&shadow)) {
        with_document(|document| document.set_component_state_dyn(shadow, state));
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

pub fn set_component_state<T: 'static>(document: &mut Document, shadow: NodeId, state: T) {
    if document.contains(shadow) {
        document.set_component_state(shadow, state);
    } else {
        PENDING_STATE.with(|cell| {
            cell.borrow_mut().insert(shadow, Box::new(state));
        });
    }
}

pub enum Prop<T> {
    Static(T),
    Dynamic(Box<dyn Fn() -> T>),
}

impl<T: 'static> Prop<T> {
    pub fn get(self) -> T {
        match self {
            Prop::Static(value) => value,
            Prop::Dynamic(read) => read(),
        }
    }

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

pub fn intrinsic(node: NodeId) -> (NodeId, Prop<ItemSize>) {
    (node, Prop::Static(ItemSize::Intrinsic))
}

pub fn fixed(node: NodeId, size: impl IntoProp<f32>) -> (NodeId, Prop<ItemSize>) {
    (node, size.into_prop().map(ItemSize::Fixed))
}

pub fn percent(node: NodeId, weight: impl IntoProp<f32>) -> (NodeId, Prop<ItemSize>) {
    (node, weight.into_prop().map(ItemSize::Percent))
}

pub struct Children(Vec<(NodeId, Prop<ItemSize>)>);

impl Children {
    fn mount(self, parent: NodeId) {
        let initial_sizes: Vec<ItemSize> = untrack(|| {
            self.0
                .iter()
                .map(|(_, size)| match size {
                    Prop::Static(size) => *size,
                    Prop::Dynamic(read) => read(),
                })
                .collect()
        });
        with_document(|document| {
            for ((child, _), size) in self.0.iter().zip(&initial_sizes) {
                document.append_child(parent, *child, *size);
            }
        });
        for (child, size) in self.0 {
            if let Prop::Dynamic(read) = size {
                create_effect(move || {
                    let size = read();
                    with_document(|document| document.set_child_size(parent, child, size));
                });
            }
        }
    }

    pub(crate) fn into_first(self) -> Option<NodeId> {
        self.0.into_iter().next().map(|(child, _)| child)
    }
}

impl<I: IntoIterator<Item = (NodeId, Prop<ItemSize>)>> From<I> for Children {
    fn from(children: I) -> Self {
        Children(children.into_iter().collect())
    }
}

pub use crate::base::click_catcher::ClickCatcherBuilder;
pub use crate::base::fill::FillBuilder;
pub use crate::base::focusable::FocusableBuilder;
pub use crate::base::outline::OutlineBuilder;
pub use crate::base::padding::PaddingBuilder;
pub use crate::base::sized::SizedBuilder;
pub use crate::base::text::TextBuilder;
pub use crate::base::visibility::VisibilityBuilder;

#[component(base)]
pub fn row(spacing: f32, children: Children) -> NodeId {
    let row = unstyled::row(spacing);
    children.mount(row);
    row
}

#[component(base)]
pub fn column(spacing: f32, children: Children) -> NodeId {
    let column = unstyled::column(spacing);
    children.mount(column);
    column
}

#[component(base)]
pub fn centered_row(spacing: f32, children: Children) -> NodeId {
    let row = unstyled::centered_row(spacing);
    children.mount(row);
    row
}

#[component(base)]
pub fn spacer() -> NodeId {
    with_document(|document| document.create_fill(Color32::TRANSPARENT, 0))
}

#[component]
pub fn show(condition: Prop<bool>, then: Option<Box<dyn FnOnce() -> NodeId>>) -> NodeId {
    let mut then = then;
    let visibility = with_document(|document| document.create_visibility(false));
    let built: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
    condition.apply(move |visible| {
        if visible && built.get().is_none() {
            let build = then.take().expect("show requires a `then` callback");
            let child = in_new_scope(build);
            built.set(Some(child));
            with_document(|document| document.set_visibility_child(visibility, child));
        }
        with_document(|document| document.set_visible(visibility, visible));
    });
    visibility
}

type ForEachView<T> = Box<dyn Fn(&T) -> (NodeId, Prop<ItemSize>)>;

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
    let parent = unstyled::column(spacing);
    let existing: Rc<RefCell<HashMap<K, (NodeId, ItemSize)>>> =
        Rc::new(RefCell::new(HashMap::new()));
    items.apply(move |items| {
        let mut existing = existing.borrow_mut();
        let mut next = Vec::with_capacity(items.len());
        for item in &items {
            let entry = existing.remove(&key(item)).unwrap_or_else(|| {
                let size: Rc<Cell<Option<ItemSize>>> = Rc::new(Cell::new(None));
                let sink = size.clone();
                let node = in_new_scope(|| {
                    let (node, item_size) = view(item);
                    sink.set(Some(item_size.get()));
                    node
                });
                (node, size.get().expect("view must set an item size"))
            });
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
    let button = match children.into_first() {
        Some(child) => {
            view! { <unstyled::button disabled={disabled} content={Box::new(move |_handle| child)} /> }
        }
        None => view! { <unstyled::button disabled={disabled} /> },
    };
    if let Some(on_click) = on_click {
        unstyled::set_button_on_click(button, on_click);
    }
    button
}
