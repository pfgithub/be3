use block::{OperationRecord, ReferenceDelta};
use uuid::Uuid;

use super::{
    lib_test_support::{counter_operation, Counter},
    BlockClient, ErasedBlock,
};

#[tokio::test]
async fn wait_until_observes_current_and_future_values() {
    let client = BlockClient::new(Uuid::new_v4());
    let block = client.create_block(Counter { count: 1 });
    block.wait_until(|counter| counter.count == 1).await;

    let block_for_update = block.clone();
    let update = tokio::spawn(async move {
        tokio::task::yield_now().await;
        block_for_update.block.remote_operation(OperationRecord {
            seq: 1,
            operation_id: Uuid::new_v4(),
            author: Uuid::new_v4(),
            operation: counter_operation(2),
            references: ReferenceDelta::default(),
        });
    });

    block.wait_until(|counter| counter.count == 3).await;
    update.await.unwrap();
}
