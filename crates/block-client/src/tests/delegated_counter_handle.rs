use futures_util::StreamExt;

use super::*;

#[tokio::test]
async fn delegated_counter_handle() {
    let account = Uuid::new_v4();
    let workspace = Uuid::new_v4();
    let id = Uuid::new_v4();
    let (endpoint, mut host) = delegated_channel();
    let client = BlockClient::delegated(account, workspace, endpoint);
    let block = client.get_block::<Counter>(id);

    assert_eq!(
        host.requests.next().await,
        Some(DelegatedRequest::Watch {
            id,
            block_type: Counter::TYPE_ID,
        })
    );
    host.events
        .unbounded_send(DelegatedEvent::Snapshot {
            id,
            block_type: Counter::TYPE_ID,
            author: account,
            sequence: 4,
            access: BlockAccess::Edit,
            data: crypto::decode(&counter_snapshot(10)),
        })
        .unwrap();
    block.loaded().await;
    assert_eq!(block.read().unwrap().count, 10);

    block.operate(CounterOperation::Add(2));
    assert_eq!(block.read().unwrap().count, 12);
    let Some(DelegatedRequest::Operate {
        operation_id,
        sequence,
        operation,
        ..
    }) = host.requests.next().await
    else {
        panic!("expected delegated operation")
    };
    assert_eq!(sequence, 5);
    assert_eq!(operation, crypto::decode(&counter_operation(2)));
    host.events
        .unbounded_send(DelegatedEvent::Acknowledged {
            id,
            operation_id,
            sequence,
        })
        .unwrap();
    host.events
        .unbounded_send(DelegatedEvent::RemoteOperation {
            id,
            operation_id: Uuid::new_v4(),
            author: Uuid::new_v4(),
            sequence: 6,
            operation: crypto::decode(&counter_operation(3)),
        })
        .unwrap();
    block.wait_until(|counter| counter.count == 15).await;

    host.events
        .unbounded_send(DelegatedEvent::RemoteOperation {
            id,
            operation_id: Uuid::new_v4(),
            author: Uuid::new_v4(),
            sequence: 6,
            operation: crypto::decode(&counter_operation(3)),
        })
        .unwrap();
    tokio::task::yield_now().await;
    assert_eq!(block.read().unwrap().count, 15);

    host.events
        .unbounded_send(DelegatedEvent::Acknowledged {
            id,
            operation_id,
            sequence,
        })
        .unwrap();
    for _ in 0..100 {
        if client.delegated_failure().is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    assert_eq!(
        client.delegated_failure().as_deref(),
        Some("stale delegated acknowledgement")
    );
}
