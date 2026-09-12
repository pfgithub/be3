use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use crate::computation::Computation;
use crate::runtime::{batch, Context, RUNTIME};

#[derive(Default)]
pub(crate) struct Owner {
    pub(crate) computation: Weak<Computation>,
    pub(crate) disposed: Cell<bool>,
    parent: Weak<Owner>,
    contexts: RefCell<HashMap<TypeId, Rc<dyn Any>>>,
    cleanups: RefCell<Vec<Box<dyn FnOnce()>>>,
}

impl Owner {
    pub(crate) fn for_computation(computation: Weak<Computation>, parent: Weak<Owner>) -> Self {
        Self {
            computation,
            parent,
            ..Self::default()
        }
    }

    pub(crate) fn add(&self, cleanup: impl FnOnce() + 'static) {
        assert!(!self.disposed.get(), "reactive scope was disposed");
        self.cleanups.borrow_mut().push(Box::new(cleanup));
    }

    pub(crate) fn is_inert(&self) -> bool {
        self.cleanups.borrow().is_empty() && self.contexts.borrow().is_empty()
    }

    pub(crate) fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        let cleanups = self.cleanups.take();
        let _context = Context::enter(Weak::new(), Weak::new());
        let mut failure = None;
        for cleanup in cleanups.into_iter().rev() {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(cleanup));
            if let Err(error) = result {
                failure.get_or_insert(error);
            }
        }
        if let Some(error) = failure {
            if !std::thread::panicking() {
                std::panic::resume_unwind(error);
            }
        }
    }
}

pub struct Scope {
    owner: Rc<Owner>,
}

impl Scope {
    pub fn new() -> Self {
        let parent = RUNTIME.with(|runtime| runtime.owner.borrow().upgrade());
        let owner = Rc::new(Owner {
            computation: parent
                .as_ref()
                .map(|parent| parent.computation.clone())
                .unwrap_or_default(),
            parent: parent.as_ref().map(Rc::downgrade).unwrap_or_default(),
            ..Owner::default()
        });
        if let Some(parent) = parent {
            let child = owner.clone();
            parent.add(move || child.dispose());
        }
        Self { owner }
    }

    pub fn run<T>(&self, f: impl FnOnce() -> T) -> T {
        assert!(!self.is_disposed(), "reactive scope was disposed");
        let _context = Context::enter(Weak::new(), Rc::downgrade(&self.owner));
        batch(f)
    }

    pub fn dispose(&self) {
        batch(|| self.owner.dispose());
    }

    pub fn is_disposed(&self) -> bool {
        self.owner.disposed.get()
    }

    pub fn context(&self) -> ScopeContext {
        ScopeContext(Rc::downgrade(&self.owner))
    }
}

#[derive(Clone)]
pub struct ScopeContext(Weak<Owner>);

impl ScopeContext {
    pub fn run<T>(&self, f: impl FnOnce() -> T) -> T {
        if let Some(owner) = self.0.upgrade() {
            assert!(!owner.disposed.get(), "reactive scope was disposed");
        }
        let _context = Context::enter(Weak::new(), self.0.clone());
        batch(f)
    }

    pub fn is_alive(&self) -> bool {
        self.0.upgrade().is_some_and(|owner| !owner.disposed.get())
    }

    pub fn child(&self) -> Option<Scope> {
        self.is_alive().then(|| self.run(Scope::new))
    }
}

pub fn owner_scope() -> Option<ScopeContext> {
    RUNTIME.with(|runtime| {
        let observer = runtime.observer.borrow().upgrade();
        match observer {
            Some(computation) => Some(ScopeContext(computation.owner.clone())),
            None => {
                let owner = runtime.owner.borrow().clone();
                owner.upgrade().map(|_| ScopeContext(owner))
            }
        }
    })
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.owner.dispose();
        } else {
            self.dispose();
        }
    }
}

pub fn on_cleanup(cleanup: impl FnOnce() + 'static) {
    current_owner().add(cleanup);
}

pub fn provide_context<T: 'static>(value: T) {
    current_owner()
        .contexts
        .borrow_mut()
        .insert(TypeId::of::<T>(), Rc::new(value));
}

pub fn use_context<T: Clone + 'static>() -> Option<T> {
    let mut owner = RUNTIME.with(|runtime| runtime.owner.borrow().upgrade());
    while let Some(current) = owner {
        let found = current
            .contexts
            .borrow()
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>().cloned());
        if found.is_some() {
            return found;
        }
        owner = current.parent.upgrade();
    }
    None
}

pub(crate) fn current_owner() -> Rc<Owner> {
    let owner = RUNTIME
        .with(|runtime| runtime.owner.borrow().upgrade())
        .expect("create memos, effects, and cleanups inside Scope::run");
    assert!(!owner.disposed.get(), "reactive scope was disposed");
    owner
}
