use std::cell::{Cell, RefCell};
use std::hash::Hash;
use std::rc::{Rc, Weak};

use crate::computation::{Computation, State};
use crate::memo::{create_memo, Memo};
use crate::runtime::{batch, RUNTIME};
use crate::selector::{create_selector, Selector};

#[derive(Default)]
pub(crate) struct Source {
    pub(crate) version: Cell<u64>,
    pub(crate) subscribers: RefCell<Vec<Weak<Computation>>>,
    pub(crate) producer: RefCell<Weak<Computation>>,
}

impl Source {
    pub(crate) fn track(self: &Rc<Self>) {
        if let Some(observer) = RUNTIME.with(|runtime| runtime.observer.borrow().upgrade()) {
            if observer.is_disposed() {
                return;
            }
            let mut dependencies = observer.dependencies.borrow_mut();
            if !dependencies
                .iter()
                .any(|(source, _)| Rc::ptr_eq(source, self))
            {
                dependencies.push((self.clone(), self.version.get()));
                self.subscribers.borrow_mut().push(Rc::downgrade(&observer));
            }
        }
    }

    pub(crate) fn changed(&self) {
        self.version.set(self.version.get().wrapping_add(1));
        let subscribers = self.subscribers.borrow().clone();
        for subscriber in subscribers {
            if let Some(subscriber) = subscriber.upgrade() {
                subscriber.invalidate(State::Dirty);
            }
        }
    }
}

pub(crate) struct Value<T> {
    pub(crate) value: RefCell<T>,
    pub(crate) source: Rc<Source>,
}

pub struct ReadSignal<T> {
    pub(crate) inner: Rc<Value<T>>,
}

pub struct WriteSignal<T> {
    inner: Rc<Value<T>>,
}

pub fn create_signal<T>(value: T) -> (ReadSignal<T>, WriteSignal<T>) {
    let inner = Rc::new(Value {
        value: RefCell::new(value),
        source: Rc::new(Source::default()),
    });
    (
        ReadSignal {
            inner: inner.clone(),
        },
        WriteSignal { inner },
    )
}

impl<T> Clone for ReadSignal<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Clone for WriteSignal<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> ReadSignal<T> {
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.inner.source.track();
        f(&self.inner.value.borrow())
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(Clone::clone)
    }

    pub fn get_untracked(&self) -> T
    where
        T: Clone,
    {
        self.inner.value.borrow().clone()
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.inner.value.borrow())
    }
}

impl<T: Clone + 'static> ReadSignal<T> {
    pub fn map<U: PartialEq + 'static>(&self, mut f: impl FnMut(T) -> U + 'static) -> Memo<U> {
        let source = self.clone();
        create_memo(move || f(source.get()))
    }

    pub fn selector(&self) -> Selector<T>
    where
        T: Eq + Hash,
    {
        let source = self.clone();
        create_selector(move || source.get())
    }
}

impl<T> WriteSignal<T> {
    pub fn set(&self, value: T)
    where
        T: PartialEq,
    {
        assert_writable();
        batch(|| {
            let mut current = self.inner.value.borrow_mut();
            if *current != value {
                *current = value;
                drop(current);
                self.inner.source.changed();
            }
        });
    }

    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        assert_writable();
        batch(|| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                f(&mut self.inner.value.borrow_mut())
            }));
            self.inner.source.changed();
            match result {
                Ok(value) => value,
                Err(error) => std::panic::resume_unwind(error),
            }
        })
    }

    pub fn set_unconditionally(&self, value: T) {
        self.update(|current| *current = value);
    }
}

fn assert_writable() {
    assert!(
        RUNTIME.with(|runtime| runtime.memo_depth.get() == 0),
        "memo computations must not write signals"
    );
}
