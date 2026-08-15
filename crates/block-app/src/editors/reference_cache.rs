use std::{collections::HashMap, sync::Arc};

use block_client::{
    block_ref::BlockRef, blocks::version_control_worktree::VersionControlWorktreeMembership,
    BlockClient,
};
use uuid::Uuid;

use crate::platform;

#[derive(Default)]
pub(super) struct ReferenceResolutionCache {
    resolved: HashMap<BlockRef, Option<Uuid>>,
    pending: Vec<(BlockRef, platform::RequestResult<Option<Uuid>>)>,
}

impl ReferenceResolutionCache {
    pub(super) fn peek(&self, reference: &BlockRef) -> Option<Uuid> {
        reference
            .as_direct()
            .or_else(|| self.resolved.get(reference).copied().flatten())
    }

    pub(super) fn poll(&mut self) {
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

    pub(super) fn resolve(
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
            let receiver = platform::spawn_request(async move {
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
    receiver: platform::RequestResult<BlockRef>,
    payload: T,
}

pub(super) struct ReferenceClassificationQueue<T> {
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
    pub(super) fn push(
        &mut self,
        client: &Arc<BlockClient>,
        referencing_id: Uuid,
        target_id: Uuid,
        payload: T,
    ) {
        let client = Arc::clone(client);
        let receiver = platform::spawn_request(async move {
            client
                .classify_reference(referencing_id, target_id, &VersionControlWorktreeMembership)
                .await
        });
        self.pending
            .push(PendingClassification { receiver, payload });
    }

    pub(super) fn poll(&mut self) -> Vec<(BlockRef, T)> {
        let mut finished = Vec::new();
        let mut still_pending = Vec::new();
        for item in self.pending.drain(..) {
            match item.receiver.try_recv() {
                Ok(reference) => finished.push((reference, item.payload)),
                Err(std::sync::mpsc::TryRecvError::Empty) => still_pending.push(item),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
            }
        }
        self.pending = still_pending;
        finished
    }
}
