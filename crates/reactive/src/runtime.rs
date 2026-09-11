use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::{Rc, Weak};

use crate::computation::Computation;
use crate::scope::Owner;

#[derive(Default)]
pub(crate) struct Runtime {
    pub(crate) observer: RefCell<Weak<Computation>>,
    pub(crate) owner: RefCell<Weak<Owner>>,
    pub(crate) queue: RefCell<VecDeque<Weak<Computation>>>,
    pub(crate) depth: Cell<usize>,
    pub(crate) flushing: Cell<bool>,
    pub(crate) memo_depth: Cell<usize>,
}

thread_local! {
    pub(crate) static RUNTIME: Runtime = Runtime::default();
}

pub(crate) struct Reset<T: Copy + 'static> {
    cell: &'static std::thread::LocalKey<Runtime>,
    field: fn(&Runtime) -> &Cell<T>,
    previous: T,
}

impl<T: Copy> Reset<T> {
    pub(crate) fn set(field: fn(&Runtime) -> &Cell<T>, value: T) -> Self {
        Self {
            cell: &RUNTIME,
            field,
            previous: RUNTIME.with(|runtime| field(runtime).replace(value)),
        }
    }
}

impl<T: Copy> Drop for Reset<T> {
    fn drop(&mut self) {
        self.cell
            .with(|runtime| (self.field)(runtime).set(self.previous));
    }
}

pub(crate) struct Context {
    observer: Weak<Computation>,
    owner: Weak<Owner>,
}

impl Context {
    pub(crate) fn enter(observer: Weak<Computation>, owner: Weak<Owner>) -> Self {
        RUNTIME.with(|runtime| Self {
            observer: runtime.observer.replace(observer),
            owner: runtime.owner.replace(owner),
        })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        RUNTIME.with(|runtime| {
            runtime.observer.replace(self.observer.clone());
            runtime.owner.replace(self.owner.clone());
        });
    }
}

pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    let owner = RUNTIME.with(|runtime| runtime.owner.borrow().clone());
    let _context = Context::enter(Weak::new(), owner);
    f()
}

pub(crate) fn tracking() -> bool {
    RUNTIME.with(|runtime| {
        runtime
            .observer
            .borrow()
            .upgrade()
            .is_some_and(|observer| !observer.is_disposed())
    })
}

pub fn batch<T>(f: impl FnOnce() -> T) -> T {
    let depth = RUNTIME.with(|runtime| runtime.depth.get());
    let guard = Reset::set(|runtime| &runtime.depth, depth + 1);
    let result = f();
    drop(guard);
    flush();
    result
}

pub fn settle<T>(f: impl FnOnce() -> T) -> T {
    let result = batch(f);
    let _guard = Reset::set(|runtime| &runtime.depth, 0);
    flush();
    result
}

pub(crate) fn enqueue(computation: &Rc<Computation>) {
    if !computation.queued.replace(true) {
        RUNTIME.with(|runtime| {
            runtime
                .queue
                .borrow_mut()
                .push_back(Rc::downgrade(computation))
        });
    }
}

pub(crate) fn flush() {
    if RUNTIME.with(|runtime| runtime.depth.get() != 0 || runtime.flushing.get()) {
        return;
    }
    let _guard = Reset::set(|runtime| &runtime.flushing, true);
    let mut runs = HashMap::new();
    loop {
        let next = RUNTIME.with(|runtime| runtime.queue.borrow_mut().pop_front());
        let Some(next) = next else { break };
        let Some(computation) = next.upgrade() else {
            continue;
        };
        computation.queued.set(false);
        let count = runs.entry(Rc::as_ptr(&computation)).or_insert(0);
        *count += 1;
        if *count > 10_000 {
            computation.dispose();
            panic!("reactive effects did not settle after 10000 executions");
        }
        computation.refresh();
    }
}
