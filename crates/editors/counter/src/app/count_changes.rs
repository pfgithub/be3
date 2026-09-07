use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Wake, Waker};

use block_client::blocks::counter::Counter;
use block_client::BlockHandle;

#[cfg(test)]
mod tests;

pub(super) struct CountChanges {
    future: Pin<Box<dyn Future<Output = ()>>>,
    value: Rc<Cell<Option<i64>>>,
    wake: Arc<CountWake>,
}

impl CountChanges {
    pub(super) fn new(block: BlockHandle<Counter>, host: block_editor_plugin::Waker) -> Self {
        let value = Rc::new(Cell::new(None));
        let changed = value.clone();
        let future = Box::pin(async move {
            block
                .wait_until(move |counter| {
                    changed.set(Some(counter.count()));
                    false
                })
                .await;
        });
        Self {
            future,
            value,
            wake: Arc::new(CountWake {
                pending: AtomicBool::new(true),
                host,
            }),
        }
    }

    pub(super) fn take(&mut self) -> Option<i64> {
        if self.wake.pending.swap(false, Ordering::AcqRel) {
            let waker = Waker::from(self.wake.clone());
            let _ = self.future.as_mut().poll(&mut Context::from_waker(&waker));
        }
        self.value.take()
    }
}

struct CountWake {
    pending: AtomicBool,
    host: block_editor_plugin::Waker,
}

impl Wake for CountWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.pending.store(true, Ordering::Release);
        self.host.wake();
    }
}
