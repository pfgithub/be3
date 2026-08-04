use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Arc},
};

use block::{Block, BlockAccess, BlockParent, BlockReference, BlockReferenceList, ClientMessage};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::{oneshot, watch};
use uuid::Uuid;

use super::{
    BlockShared, CachedBlock, ClientDebugSnapshot, DeferredRequest, ErasedBlock,
    NetworkDebugSnapshot, PendingRequest, ReferenceListShared, TypedBlock, WorkerState,
};

#[derive(Clone, Deserialize, Serialize)]
struct DebugBlock;

impl Block for DebugBlock {
    type Operation = ();
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0xfeed);

    fn apply_operation(_block: &mut Self, _operation: &Self::Operation) {}

    fn implicit_name(&self) -> String {
        "Debug block".into()
    }
}

fn block(id: Uuid, ready: bool) -> Arc<dyn ErasedBlock> {
    let block: Arc<dyn ErasedBlock> = Arc::new(TypedBlock::created(
        id,
        Arc::new(BlockShared {
            value: RwLock::new(Some(DebugBlock)),
        }),
        DebugBlock,
    ));
    if ready {
        block.created();
    }
    block
}

#[test]
fn client_debug_snapshot_reports_active_worker_state() {
    let client_id = Uuid::from_u128(1);
    let debug = Arc::new(RwLock::new(ClientDebugSnapshot::empty(
        client_id,
        Uuid::from_u128(2),
        Uuid::from_u128(3),
    )));
    let cache = Arc::new(RwLock::new(HashMap::new()));
    let mut state = WorkerState::new(
        Arc::new(RwLock::new(())),
        Arc::new(RwLock::new(NetworkDebugSnapshot::default())),
        Arc::clone(&debug),
        Arc::clone(&cache),
        Arc::new(RwLock::new(HashMap::new())),
    );
    state.connected = true;
    state.sending_paused = true;
    state.steps_remaining = 2;

    let later_block_id = Uuid::from_u128(20);
    let earlier_block_id = Uuid::from_u128(10);
    state
        .blocks
        .insert(later_block_id, block(later_block_id, false));
    state
        .blocks
        .insert(earlier_block_id, block(earlier_block_id, true));

    let reference_id = Uuid::from_u128(30);
    let loaded = watch::channel(false).0;
    loaded.send_replace(true);
    state.reference_lists.insert(
        BlockReferenceList::Roots,
        Arc::new(ReferenceListShared {
            blocks: RwLock::new(vec![BlockReference {
                id: reference_id,
                block_type: DebugBlock::TYPE_ID,
                author: Uuid::new_v4(),
                name: "Referenced".into(),
                parent: BlockParent::Root,
                references: 0,
                dynamic_artifact: false,
                access: BlockAccess::Edit,
            }]),
            loaded,
            subscribers: AtomicUsize::new(1),
        }),
    );

    cache.write().insert(
        reference_id,
        CachedBlock {
            id: reference_id,
            block_type: DebugBlock::TYPE_ID,
            author: Uuid::new_v4(),
            name: "Referenced".into(),
        },
    );

    let later_request_id = Uuid::from_u128(50);
    let earlier_request_id = Uuid::from_u128(40);
    state.requests.insert(
        later_request_id,
        PendingRequest::Read { id: later_block_id },
    );
    state.requests.insert(
        earlier_request_id,
        PendingRequest::Create {
            id: earlier_block_id,
        },
    );
    state.outbound.push_back(ClientMessage::ReadBlock {
        request_id: later_request_id,
        id: later_block_id,
        watch: true,
    });
    state.deferred.push_back(DeferredRequest::SetBlockName {
        id: earlier_block_id,
        name: "Renamed".into(),
    });
    let (completed, _completion) = oneshot::channel();
    state.synchronization_waiters.push(completed);

    state.refresh_debug();
    let snapshot = debug.read().clone();

    assert_eq!(snapshot.client_id, client_id);
    assert!(snapshot.connected);
    assert!(snapshot.sending_paused);
    assert_eq!(snapshot.queued_messages, 1);
    assert_eq!(snapshot.steps_remaining, 2);
    assert_eq!(snapshot.synchronization_waiters, 1);
    assert!(!snapshot.changes_saved);
    assert_eq!(
        snapshot
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![earlier_block_id, later_block_id]
    );
    assert!(snapshot.blocks[0].ready);
    assert!(snapshot.blocks[0].synchronized);
    assert!(!snapshot.blocks[1].ready);
    assert!(snapshot.blocks[1].has_local_changes);
    assert_eq!(snapshot.reference_lists.len(), 1);
    assert!(snapshot.reference_lists[0].loaded);
    assert_eq!(snapshot.reference_lists[0].blocks, 1);
    assert_eq!(snapshot.cached_blocks[0].id, reference_id);
    assert_eq!(
        snapshot
            .pending_requests
            .iter()
            .map(|request| request.request_id)
            .collect::<Vec<_>>(),
        vec![earlier_request_id, later_request_id]
    );
    assert_eq!(snapshot.outbound_messages[0].kind, "Read block");
    assert_eq!(snapshot.deferred_requests[0].kind, "Set block name");
}
