use super::*;

#[tokio::test]
async fn batched_updates_are_observed_together() {
    let data_dir = std::env::temp_dir().join(format!("block-e2e-batch-test-{}", Uuid::new_v4()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server_data_dir = data_dir.clone();
    let server = tokio::spawn(async move {
        block_server::serve(listener, server_data_dir)
            .await
            .unwrap();
    });
    let url = format!("http://{address}");
    let (account_id, workspace_id) = test_identity(&url).await;

    let client_a = BlockClient::new(account_id, workspace_id);
    client_a.connect(url.clone());
    let first_a = client_a.create_block(Counter { count: 0 });
    let second_a = client_a.create_block(Counter { count: 0 });
    timeout(first_a.loaded()).await;
    timeout(second_a.loaded()).await;

    let client_b = BlockClient::new(account_id, workspace_id);
    client_b.connect(url);
    let first_b = client_b.get_block::<Counter>(first_a.id());
    let second_b = client_b.get_block::<Counter>(second_a.id());
    timeout(first_b.loaded()).await;
    timeout(second_b.loaded()).await;

    let observed_second = second_b.clone();
    let observation = tokio::spawn(async move {
        first_b
            .wait_until(|counter| counter.count == 1 && observed_second.read().unwrap().count == 2)
            .await;
    });

    client_a.batch(|batch| {
        batch.operate(&first_a, CounterOperation::Add(1));
        batch.operate(&second_a, CounterOperation::Add(2));
    });
    timeout(client_a.synchronized()).await;
    tokio::time::timeout(Duration::from_secs(2), observation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second_b.read().unwrap().count, 2);

    drop(first_a);
    drop(second_a);
    drop(second_b);
    drop(client_a);
    drop(client_b);
    server.abort();
    let _ = server.await;
    fs::remove_dir_all(data_dir).await.unwrap();
}
