use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::runtime::{batch, enqueue, flush, Context, Reset, RUNTIME};
use crate::scope::{current_owner, Owner};
use crate::signal::Source;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    Clean,
    Check,
    Dirty,
    Disposed,
}

type Callback = Box<dyn FnMut() -> bool>;

pub(crate) struct Computation {
    parent: Weak<Computation>,
    pub(crate) owner: Weak<Owner>,
    state: Cell<State>,
    refreshing: Cell<bool>,
    callback: RefCell<Option<Callback>>,
    execution_owner: RefCell<Rc<Owner>>,
    pub(crate) dependencies: RefCell<Vec<(Rc<Source>, u64)>>,
    pub(crate) source: Option<Rc<Source>>,
    pub(crate) queued: Cell<bool>,
}

impl Computation {
    pub(crate) fn new(source: Option<Rc<Source>>, callback: Callback) -> Rc<Self> {
        let owner = current_owner();
        let computation = Rc::new(Self {
            parent: owner.computation.clone(),
            owner: Rc::downgrade(&owner),
            state: Cell::new(State::Dirty),
            refreshing: Cell::new(false),
            callback: RefCell::new(Some(callback)),
            execution_owner: RefCell::new(Rc::new(Owner::default())),
            dependencies: RefCell::new(Vec::new()),
            source,
            queued: Cell::new(false),
        });
        let owned = computation.clone();
        owner.add(move || owned.dispose());
        computation
    }

    pub(crate) fn invalidate(self: &Rc<Self>, state: State) {
        let previous = self.state.get();
        if previous == State::Disposed {
            return;
        }
        if self.source.is_none() {
            enqueue(self);
        }
        if previous == State::Dirty || previous == state {
            return;
        }
        self.state.set(state);
        if let Some(source) = &self.source {
            let subscribers = source.subscribers.borrow().clone();
            for subscriber in subscribers {
                if let Some(subscriber) = subscriber.upgrade() {
                    subscriber.invalidate(State::Check);
                }
            }
        }
    }

    pub(crate) fn refresh(self: &Rc<Self>) {
        if self.source.is_none() {
            if let Some(parent) = self.parent.upgrade() {
                if !parent.refreshing.get() {
                    parent.refresh();
                }
            }
        }
        assert!(!self.refreshing.get(), "reactive dependency cycle detected");
        if matches!(self.state.get(), State::Clean | State::Disposed) {
            return;
        }
        batch(|| {
            self.refreshing.set(true);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.refresh_inner();
            }));
            self.refreshing.set(false);
            if let Err(error) = result {
                if self.state.get() != State::Disposed {
                    self.state.set(State::Dirty);
                    if self.source.is_none() {
                        enqueue(self);
                    }
                }
                std::panic::resume_unwind(error);
            }
        });
    }

    fn refresh_inner(self: &Rc<Self>) {
        if self.state.get() == State::Check {
            let dependencies = self.dependencies.borrow().clone();
            for (source, version) in dependencies {
                let producer = source.producer.borrow().upgrade();
                if let Some(producer) = producer {
                    producer.refresh();
                }
                if source.version.get() != version {
                    self.state.set(State::Dirty);
                    break;
                }
            }
            if self.state.get() == State::Check {
                self.state.set(State::Clean);
                return;
            }
        }
        self.detach();
        let old_owner = self.execution_owner.replace(Rc::new(Owner::for_computation(
            Rc::downgrade(self),
            self.owner.clone(),
        )));
        old_owner.dispose();
        if self.state.get() == State::Disposed {
            return;
        }
        let owner = self.execution_owner.borrow().clone();
        let _context = Context::enter(Rc::downgrade(self), Rc::downgrade(&owner));
        let memo_depth = RUNTIME.with(|runtime| runtime.memo_depth.get());
        let _memo_guard = Reset::set(
            |runtime| &runtime.memo_depth,
            memo_depth + usize::from(self.source.is_some()),
        );
        self.state.set(State::Clean);
        let mut callback = self
            .callback
            .take()
            .expect("reactive callback is already running");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut callback));
        if self.state.get() != State::Disposed {
            self.callback.replace(Some(callback));
        }
        match result {
            Ok(changed) => {
                if changed {
                    if let Some(source) = &self.source {
                        source.version.set(source.version.get().wrapping_add(1));
                    }
                }
            }
            Err(error) => std::panic::resume_unwind(error),
        }
        if self.is_spent() {
            self.dispose();
        }
    }

    fn is_spent(&self) -> bool {
        self.source.is_none()
            && self.state.get() == State::Clean
            && self.dependencies.borrow().is_empty()
            && self.execution_owner.borrow().is_inert()
    }

    fn detach(&self) {
        for (source, _) in self.dependencies.take() {
            source.subscribers.borrow_mut().retain(|subscriber| {
                !std::ptr::eq(subscriber.as_ptr(), self) && subscriber.strong_count() > 0
            });
        }
    }

    pub(crate) fn dispose(&self) {
        if self.state.replace(State::Disposed) == State::Disposed {
            return;
        }
        self.detach();
        self.callback.take();
        let owner = self.execution_owner.borrow().clone();
        owner.dispose();
    }

    pub(crate) fn is_disposed(&self) -> bool {
        self.state.get() == State::Disposed
    }
}

#[derive(Clone)]
pub struct Effect {
    computation: Rc<Computation>,
}

pub fn create_effect(mut effect: impl FnMut() + 'static) -> Effect {
    let computation = Computation::new(
        None,
        Box::new(move || {
            effect();
            false
        }),
    );
    enqueue(&computation);
    flush();
    Effect { computation }
}

impl Effect {
    pub fn dispose(&self) {
        batch(|| self.computation.dispose());
    }

    pub fn is_disposed(&self) -> bool {
        self.computation.is_disposed()
    }
}
