use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use crate::arena::{self, SlotId};
use crate::computation::{Computation, State};
use crate::runtime::{batch, RUNTIME};

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
    id: SlotId,
    _marker: PhantomData<fn() -> T>,
}

pub struct WriteSignal<T> {
    id: SlotId,
    _marker: PhantomData<fn(T)>,
}

pub fn create_signal<T: 'static>(value: T) -> (ReadSignal<T>, WriteSignal<T>) {
    let id = arena::insert(Value {
        value: RefCell::new(value),
        source: Rc::new(Source::default()),
    });
    (
        ReadSignal {
            id,
            _marker: PhantomData,
        },
        WriteSignal {
            id,
            _marker: PhantomData,
        },
    )
}

impl<T> Clone for ReadSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ReadSignal<T> {}

impl<T> Clone for WriteSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for WriteSignal<T> {}

impl<T: 'static> ReadSignal<T> {
    pub(crate) fn inner(&self) -> Rc<Value<T>> {
        arena::get::<Value<T>>(self.id)
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let inner = self.inner();
        inner.source.track();
        let result = f(&inner.value.borrow());
        result
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
        self.inner().value.borrow().clone()
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let inner = self.inner();
        let result = f(&inner.value.borrow());
        result
    }
}

impl<T: 'static> WriteSignal<T> {
    fn inner(&self) -> Rc<Value<T>> {
        arena::get::<Value<T>>(self.id)
    }

    pub fn set(&self, value: T)
    where
        T: PartialEq,
    {
        assert_writable();
        let inner = self.inner();
        batch(|| {
            let mut current = inner.value.borrow_mut();
            if *current != value {
                *current = value;
                drop(current);
                inner.source.changed();
            }
        });
    }

    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        assert_writable();
        let inner = self.inner();
        batch(|| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                f(&mut inner.value.borrow_mut())
            }));
            inner.source.changed();
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
