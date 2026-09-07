use std::cell::RefCell;
use std::rc::Rc;

use crate::computation::Computation;
use crate::runtime::untrack;
use crate::signal::{Source, Value};

pub struct Memo<T> {
    inner: Rc<Value<Option<T>>>,
    computation: Rc<Computation>,
}

pub fn create_memo<T: PartialEq + 'static>(mut compute: impl FnMut() -> T + 'static) -> Memo<T> {
    let source = Rc::new(Source::default());
    let inner = Rc::new(Value {
        value: RefCell::new(None),
        source: source.clone(),
    });
    let output = inner.clone();
    let computation = Computation::new(
        Some(source.clone()),
        Box::new(move || {
            let next = compute();
            let mut value = output.value.borrow_mut();
            if value.as_ref() == Some(&next) {
                false
            } else {
                *value = Some(next);
                true
            }
        }),
    );
    source.producer.replace(Rc::downgrade(&computation));
    computation.refresh();
    Memo { inner, computation }
}

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            computation: self.computation.clone(),
        }
    }
}

impl<T> Memo<T> {
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        assert!(
            !self.computation.is_disposed(),
            "reactive memo was disposed"
        );
        self.computation.refresh();
        self.inner.source.track();
        f(self
            .inner
            .value
            .borrow()
            .as_ref()
            .expect("memo has no value"))
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
        untrack(|| self.get())
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        untrack(|| self.with(f))
    }
}
