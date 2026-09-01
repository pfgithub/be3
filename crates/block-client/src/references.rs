use std::{
    collections::HashMap,
    future::Future,
    sync::{
        mpsc::{self, Receiver},
        Arc,
    },
};

use uuid::Uuid;

use crate::{
    block_ref::BlockRef, blocks::version_control_worktree::VersionControlWorktreeMembership,
    BlockClient,
};

pub type RequestResult<T> = Receiver<T>;

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_request<T>(future: impl Future<Output = T> + Send + 'static) -> RequestResult<T>
where
    T: Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    crate::transport::spawn_worker(async move {
        let _ = sender.send(future.await);
    });
    receiver
}

#[cfg(target_arch = "wasm32")]
pub fn spawn_request<T>(future: impl Future<Output = T> + 'static) -> RequestResult<T>
where
    T: 'static,
{
    let (sender, receiver) = mpsc::channel();
    crate::transport::spawn_worker(async move {
        let _ = sender.send(future.await);
    });
    receiver
}

#[derive(Default)]
pub struct ReferenceResolutionCache {
    resolved: HashMap<BlockRef, Option<Uuid>>,
    pending: Vec<(BlockRef, RequestResult<Option<Uuid>>)>,
}

impl ReferenceResolutionCache {
    pub fn peek(&self, reference: &BlockRef) -> Option<Uuid> {
        reference
            .as_direct()
            .or_else(|| self.resolved.get(reference).copied().flatten())
    }

    pub fn poll(&mut self) {
        let mut finished = Vec::new();
        self.pending
            .retain(|(reference, receiver)| match receiver.try_recv() {
                Ok(resolved) => {
                    finished.push((*reference, resolved));
                    false
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
            });
        for (reference, resolved) in finished {
            self.resolved.insert(reference, resolved);
        }
    }

    pub fn resolve(
        &mut self,
        client: &Arc<BlockClient>,
        referencing_id: Uuid,
        reference: BlockRef,
    ) -> Option<Uuid> {
        if let Some(id) = reference.as_direct() {
            return Some(id);
        }
        if let Some(resolved) = self.resolved.get(&reference) {
            return *resolved;
        }
        if !self
            .pending
            .iter()
            .any(|(pending, _)| *pending == reference)
        {
            let client = Arc::clone(client);
            let receiver = spawn_request(async move {
                client
                    .resolve_reference(
                        referencing_id,
                        &reference,
                        &VersionControlWorktreeMembership,
                    )
                    .await
            });
            self.pending.push((reference, receiver));
        }
        None
    }
}

struct PendingClassification<T> {
    receiver: RequestResult<BlockRef>,
    payload: T,
}

pub struct ReferenceClassificationQueue<T> {
    pending: Vec<PendingClassification<T>>,
}

impl<T> Default for ReferenceClassificationQueue<T> {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
        }
    }
}

impl<T> ReferenceClassificationQueue<T> {
    pub fn push(
        &mut self,
        client: &Arc<BlockClient>,
        referencing_id: Uuid,
        target_id: Uuid,
        payload: T,
    ) {
        let client = Arc::clone(client);
        let receiver = spawn_request(async move {
            client
                .classify_reference(referencing_id, target_id, &VersionControlWorktreeMembership)
                .await
        });
        self.pending
            .push(PendingClassification { receiver, payload });
    }

    pub fn poll(&mut self) -> Vec<(BlockRef, T)> {
        self.poll_with_failures().0
    }

    pub fn poll_with_failures(&mut self) -> (Vec<(BlockRef, T)>, Vec<T>) {
        let mut finished = Vec::new();
        let mut failed = Vec::new();
        let mut still_pending = Vec::new();
        for item in self.pending.drain(..) {
            match item.receiver.try_recv() {
                Ok(reference) => finished.push((reference, item.payload)),
                Err(std::sync::mpsc::TryRecvError::Empty) => still_pending.push(item),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => failed.push(item.payload),
            }
        }
        self.pending = still_pending;
        (finished, failed)
    }
}
