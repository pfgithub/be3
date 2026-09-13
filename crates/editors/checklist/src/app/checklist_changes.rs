use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Wake, Waker};

use block_client::blocks::checklist::Checklist;
use block_client::BlockHandle;

use super::ChecklistSnapshot;

pub(super) struct ChecklistChanges {
    future: Pin<Box<dyn Future<Output = ()>>>,
    snapshot: Rc<RefCell<Option<ChecklistSnapshot>>>,
    wake: Arc<ChecklistWake>,
}

impl ChecklistChanges {
    pub(super) fn new(block: BlockHandle<Checklist>, host: block_editor_plugin::Waker) -> Self {
        let snapshot = Rc::new(RefCell::new(None));
        let changed = snapshot.clone();
        let future = Box::pin(async move {
            block
                .wait_until(move |checklist| {
                    *changed.borrow_mut() = Some(ChecklistSnapshot::from(checklist));
                    false
                })
                .await;
        });
        Self {
            future,
            snapshot,
            wake: Arc::new(ChecklistWake {
                pending: AtomicBool::new(true),
                host,
            }),
        }
    }

    pub(super) fn take(&mut self) -> Option<ChecklistSnapshot> {
        if self.wake.pending.swap(false, Ordering::AcqRel) {
            let waker = Waker::from(self.wake.clone());
            let _ = self.future.as_mut().poll(&mut Context::from_waker(&waker));
        }
        self.snapshot.borrow_mut().take()
    }
}

struct ChecklistWake {
    pending: AtomicBool,
    host: block_editor_plugin::Waker,
}

impl Wake for ChecklistWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.pending.store(true, Ordering::Release);
        self.host.wake();
    }
}
