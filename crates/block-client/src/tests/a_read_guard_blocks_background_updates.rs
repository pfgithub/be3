use std::{sync::mpsc, sync::Arc, thread};

use block::{OperationRecord, ReferenceDelta};
use parking_lot::RwLock;
use uuid::Uuid;

use super::{
    lib_test_support::{counter_operation, Counter},
    BlockShared, ErasedBlock, TypedBlock,
};

#[test]
fn a_read_guard_blocks_background_updates() {
    let shared = Arc::new(BlockShared {
        value: RwLock::new(Some(Counter { count: 0 })),
    });
    let block = Arc::new(TypedBlock::<Counter>::created(
        Uuid::new_v4(),
        Arc::clone(&shared),
        Counter { count: 0 },
    ));
    block.created();
    let read = shared.value.read();
    let block_for_thread = Arc::clone(&block);
    let (finished_tx, finished_rx) = mpsc::channel();
    let update = thread::spawn(move || {
        block_for_thread.remote_operation(OperationRecord {
            seq: 1,
            operation_id: Uuid::new_v4(),
            author: Uuid::new_v4(),
            operation: counter_operation(1),
            references: ReferenceDelta::default(),
        });
        finished_tx.send(()).unwrap();
    });

    assert!(finished_rx.try_recv().is_err());
    drop(read);
    finished_rx.recv().unwrap();
    update.join().unwrap();
    assert_eq!(shared.value.read().as_ref().unwrap().count, 1);
}
