use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::arena::{self, SlotId};
use crate::computation::Computation;
use crate::runtime::untrack;
use crate::signal::{Source, Value};

struct MemoInner<T> {
    value: Rc<Value<Option<T>>>,
    computation: Rc<Computation>,
}

pub struct Memo<T> {
    id: SlotId,
    _marker: PhantomData<fn() -> T>,
}

pub fn create_memo<T: PartialEq + 'static>(mut compute: impl FnMut() -> T + 'static) -> Memo<T> {
    let source = Rc::new(Source::default());
    let value = Rc::new(Value {
        value: RefCell::new(None),
        source: source.clone(),
    });
    let output = value.clone();
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
    let id = arena::insert(MemoInner { value, computation });
    Memo {
        id,
        _marker: PhantomData,
    }
}

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Memo<T> {}

impl<T: 'static> Memo<T> {
    fn inner(&self) -> Rc<MemoInner<T>> {
        arena::get::<MemoInner<T>>(self.id)
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let inner = self.inner();
        assert!(
            !inner.computation.is_disposed(),
            "reactive memo was disposed"
        );
        inner.computation.refresh();
        inner.value.source.track();
        let value = inner.value.value.borrow();
        let result = f(value.as_ref().expect("memo has no value"));
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
        untrack(|| self.get())
    }

    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        untrack(|| self.with(f))
    }
}
