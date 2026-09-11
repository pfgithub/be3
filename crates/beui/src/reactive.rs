use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use crate::base::{Align, Direction, ItemSize};
use crate::color::Color32;
use crate::document::Document;
use crate::geometry::Vec2;
use crate::node::{ClickHandler, Handler, NodeId};
use crate::unstyled;

pub use beui_macros::{component, view};
pub use reactive::{
    batch, create_effect, create_memo, create_selector, create_signal, on_cleanup, owner_scope,
    provide_context, settle, untrack, use_context, Effect, Memo, ReadSignal, Scope, ScopeContext,
    Selector, WriteSignal,
};

thread_local! {
    static CURRENT_DOCUMENT: RefCell<Option<Document>> = const { RefCell::new(None) };
    static ACTIVE_DOCUMENT: Cell<*mut Document> = const { Cell::new(std::ptr::null_mut()) };
    static CURRENT_COMPONENT: Cell<Option<NodeId>> = const { Cell::new(None) };
    static COMPONENT_NAME: Cell<Option<&'static str>> = const { Cell::new(None) };
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

pub fn node_size(node: NodeId) -> ReadSignal<Vec2> {
    with_document(|document| document.watch_size(node))
}

pub fn copy_text(text: impl Into<String>) {
    let text = text.into();
    with_document(|document| document.copy_text(text));
}

pub(crate) fn node_scope(document: &Document, owner: Option<ScopeContext>) -> Scope {
    owner
        .or_else(owner_scope)
        .and_then(|owner| owner.child())
        .unwrap_or_else(|| document.reactive_scope().context().run(Scope::new))
}

pub fn in_new_scope(f: impl FnOnce() -> NodeId) -> NodeId {
    let scope = with_document(|document| node_scope(document, None));
    let node = scope.context().run(f);
    with_document(|document| document.register_node_scope(node, scope));
    node
}

pub fn set_component_name(name: &'static str) {
    COMPONENT_NAME.with(|cell| cell.set(Some(name)));
}

pub fn component(name: &'static str, f: impl FnOnce() -> NodeId) -> NodeId {
    let shadow = with_document(Document::reserve_shadow);
    let scope = Scope::new();
    let outer_name = COMPONENT_NAME.with(|cell| cell.take());
    let root = in_component(Some(shadow), || scope.run(f));
    let name = COMPONENT_NAME
        .with(|cell| cell.replace(outer_name))
        .unwrap_or(name);
    with_document(|document| {
        document.finish_shadow(shadow, name, root, Vec::new());
        document.register_node_scope(shadow, scope);
    });
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
        .expect("a component binding was used outside of a #[component] body")
}

fn set_detail(shadow: NodeId, detail: String) {
    with_document(|document| {
        if document.contains(shadow) {
            document.set_component_detail(shadow, detail);
        } else {
            PENDING_DETAIL.with(|cell| {
                cell.borrow_mut().insert(shadow, detail);
            });
        }
    });
}

pub fn component_detail(detail: impl Fn() -> String + 'static) {
    let shadow = current_component();
    create_effect(move || set_detail(shadow, detail()));
}

pub fn set_component_state<T: 'static>(state: T) {
    let shadow = current_component();
    with_document(|document| {
        if document.contains(shadow) {
            document.set_component_state(shadow, state);
        } else {
            PENDING_STATE.with(|cell| {
                cell.borrow_mut().insert(shadow, Box::new(state));
            });
        }
    });
}

pub fn component_state<T: 'static, R>(shadow: NodeId, read: impl FnOnce(&T) -> R) -> R {
    with_document(|document| read(document.component_state::<T>(shadow)))
}

pub fn component_state_mut<T: 'static, R>(shadow: NodeId, write: impl FnOnce(&mut T) -> R) -> R {
    with_document(|document| write(document.component_state_mut::<T>(shadow)))
}

#[derive(Clone, Default)]
pub struct NodeRef(Rc<Cell<Option<NodeId>>>);

impl NodeRef {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fill(&self, node: NodeId) {
        self.0.set(Some(node));
    }

    pub fn get(&self) -> NodeId {
        self.0
            .get()
            .expect("node_ref read before the node it points at was built")
    }

    pub fn try_get(&self) -> Option<NodeId> {
        self.0.get()
    }
}

pub struct Callback<V, R = ()>(Rc<RefCell<Option<Handler<V, R>>>>);

impl<V, R> Callback<V, R> {
    pub fn new(handler: impl FnMut(V) -> R + 'static) -> Self {
        Self(Rc::new(RefCell::new(Some(Box::new(handler)))))
    }

    pub fn empty() -> Self {
        Self(Rc::new(RefCell::new(None)))
    }

    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_none()
    }

    pub fn set(&self, handler: impl FnMut(V) -> R + 'static) {
        *self.0.borrow_mut() = Some(Box::new(handler));
    }

    pub fn call(&self, value: V) -> R
    where
        R: Default,
    {
        let Some(mut handler) = self.0.borrow_mut().take() else {
            return R::default();
        };
        let result = handler(value);
        let mut slot = self.0.borrow_mut();
        if slot.is_none() {
            *slot = Some(handler);
        }
        result
    }
}

impl<V, R> Clone for Callback<V, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<V, R> Default for Callback<V, R> {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Default)]
pub struct ClickCallback(Rc<RefCell<Option<ClickHandler>>>);

impl ClickCallback {
    pub fn new(handler: impl FnMut() + 'static) -> Self {
        Self(Rc::new(RefCell::new(Some(Box::new(handler)))))
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_none()
    }

    pub fn set(&self, handler: impl FnMut() + 'static) {
        *self.0.borrow_mut() = Some(Box::new(handler));
    }

    pub fn call(&self) {
        let Some(mut handler) = self.0.borrow_mut().take() else {
            return;
        };
        handler();
        let mut slot = self.0.borrow_mut();
        if slot.is_none() {
            *slot = Some(handler);
        }
    }
}

struct ComponentGuard(Option<NodeId>);

impl Drop for ComponentGuard {
    fn drop(&mut self) {
        CURRENT_COMPONENT.with(|cell| cell.set(self.0));
    }
}

fn in_component<R>(owner: Option<NodeId>, f: impl FnOnce() -> R) -> R {
    let _guard = ComponentGuard(CURRENT_COMPONENT.with(|cell| cell.replace(owner)));
    f()
}

pub struct Render<H = ()> {
    owner: Option<NodeId>,
    render: Box<dyn FnOnce(H) -> NodeId>,
}

impl<H> Render<H> {
    pub fn new(render: impl FnOnce(H) -> NodeId + 'static) -> Self {
        Self {
            owner: CURRENT_COMPONENT.with(Cell::get),
            render: Box::new(render),
        }
    }

    pub fn call(self, handle: H) -> NodeId {
        in_component(self.owner, || (self.render)(handle))
    }
}

pub struct RenderFn<H> {
    owner: Option<NodeId>,
    render: Rc<dyn Fn(H) -> NodeId>,
}

impl<H> RenderFn<H> {
    pub fn new(render: impl Fn(H) -> NodeId + 'static) -> Self {
        Self {
            owner: CURRENT_COMPONENT.with(Cell::get),
            render: Rc::new(render),
        }
    }

    pub fn call(&self, handle: H) -> NodeId {
        in_component(self.owner, || (self.render)(handle))
    }
}

impl<H> Clone for RenderFn<H> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner,
            render: self.render.clone(),
        }
    }
}

pub struct Func<V, R>(Rc<dyn Fn(V) -> R>);

impl<V, R> Func<V, R> {
    pub fn new(function: impl Fn(V) -> R + 'static) -> Self {
        Self(Rc::new(function))
    }

    pub fn call(&self, value: V) -> R {
        (self.0)(value)
    }
}

impl<V, R> Clone for Func<V, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub trait IntoFunc<V, R> {
    fn into_func(self) -> Func<V, R>;
}

impl<V, R, F: Fn(V) -> R + 'static> IntoFunc<V, R> for F {
    fn into_func(self) -> Func<V, R> {
        Func::new(self)
    }
}

impl<V, R> IntoFunc<V, R> for Func<V, R> {
    fn into_func(self) -> Func<V, R> {
        self
    }
}

pub trait IntoRender<H> {
    fn into_render(self) -> Render<H>;
}

impl<H, F: FnOnce(H) -> NodeId + 'static> IntoRender<H> for F {
    fn into_render(self) -> Render<H> {
        Render::new(self)
    }
}

impl<H> IntoRender<H> for Render<H> {
    fn into_render(self) -> Render<H> {
        self
    }
}

pub trait IntoRenderFn<H> {
    fn into_render_fn(self) -> RenderFn<H>;
}

impl<H, F: Fn(H) -> NodeId + 'static> IntoRenderFn<H> for F {
    fn into_render_fn(self) -> RenderFn<H> {
        RenderFn::new(self)
    }
}

impl<H> IntoRenderFn<H> for RenderFn<H> {
    fn into_render_fn(self) -> RenderFn<H> {
        self
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

    pub fn peek(&self) -> T
    where
        T: Clone,
    {
        untrack(|| match self {
            Prop::Static(value) => value.clone(),
            Prop::Dynamic(read) => read(),
        })
    }

    pub fn reader(self) -> Box<dyn Fn() -> T>
    where
        T: Clone,
    {
        match self {
            Prop::Static(value) => Box::new(move || value.clone()),
            Prop::Dynamic(read) => read,
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

#[derive(Default)]
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

    pub(crate) fn mount_scroll_items(self, scroll: NodeId) {
        with_document(|document| {
            for (child, _) in &self.0 {
                document.append_scroll_item(scroll, *child);
            }
        });
    }

    pub(crate) fn into_first(self) -> Option<NodeId> {
        self.0.into_iter().next().map(|(child, _)| child)
    }

    pub(crate) fn into_items(self) -> Vec<(NodeId, Prop<ItemSize>)> {
        self.0
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
pub use crate::base::scroll::{ScrollBuilder, VirtualListBuilder};
pub use crate::base::sized::SizedBuilder;
pub use crate::base::text::TextBuilder;
pub use crate::base::visibility::VisibilityBuilder;

#[component(base)]
pub fn list(
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    align: Option<Align>,
    spacing: f32,
    children: Children,
) -> NodeId {
    let list = with_document(|document| {
        let list = document.create_list(direction.peek(), spacing);
        if let Some(align) = align {
            document.set_list_align(list, align);
        }
        list
    });
    direction.apply(move |direction| {
        with_document(|document| document.set_list_direction(list, direction));
    });
    children.mount(list);
    list
}

#[component(base)]
pub fn row(spacing: f32, children: Children) -> NodeId {
    view! { <list direction={Direction::Horizontal} spacing={spacing} children={children} /> }
}

#[component(base)]
pub fn column(spacing: f32, children: Children) -> NodeId {
    view! { <list direction={Direction::Vertical} spacing={spacing} children={children} /> }
}

#[component(base)]
pub fn centered_row(spacing: f32, children: Children) -> NodeId {
    view! {
        <list
            direction={Direction::Horizontal}
            align={Align::Center}
            spacing={spacing}
            children={children}
        />
    }
}

#[component(base)]
pub fn spacer() -> NodeId {
    with_document(|document| document.create_fill(Color32::TRANSPARENT, 0))
}

#[component]
pub fn show(condition: Prop<bool>, then: Option<Render>) -> NodeId {
    let mut then = then;
    let visibility = with_document(|document| document.create_visibility(false));
    let built: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
    condition.apply(move |visible| {
        if visible && built.get().is_none() {
            let build = then.take().expect("show requires a `then` callback");
            let child = in_new_scope(|| build.call(()));
            built.set(Some(child));
            with_document(|document| document.set_visibility_child(visibility, child));
        }
        with_document(|document| document.set_visible(visibility, visible));
    });
    visibility
}

#[component]
pub fn for_each<T, K>(
    spacing: f32,
    items: Prop<Vec<T>>,
    key: Option<Func<T, K>>,
    view: Option<RenderFn<T>>,
    #[prop(default = ItemSize::Intrinsic)] item_size: ItemSize,
) -> NodeId
where
    T: Clone + 'static,
    K: Hash + Eq + 'static,
{
    let key = key.expect("for_each requires a `key` callback");
    let view = view.expect("for_each requires a `view` callback");
    let parent = view! { <column spacing={spacing} /> };
    let existing: Rc<RefCell<HashMap<K, NodeId>>> = Rc::new(RefCell::new(HashMap::new()));
    items.apply(move |items| {
        let mut existing = existing.borrow_mut();
        let mut next = Vec::with_capacity(items.len());
        for item in &items {
            let node = existing
                .remove(&key.call(item.clone()))
                .unwrap_or_else(|| in_new_scope(|| view.call(item.clone())));
            next.push((key.call(item.clone()), node));
        }
        with_document(|document| {
            for removed in existing.values() {
                document.remove_child(parent, *removed);
                document.remove_node(*removed);
            }
            for (_, child) in &next {
                document.remove_child(parent, *child);
            }
            for (_, child) in &next {
                document.append_child(parent, *child, item_size);
            }
        });
        existing.clear();
        existing.extend(next);
    });
    parent
}

#[component]
pub fn button(children: Children, disabled: Prop<bool>, on_click: ClickCallback) -> NodeId {
    view! {
        <unstyled::button
            disabled={disabled}
            on_click={move || on_click.call()}
            children={children}
        />
    }
}
