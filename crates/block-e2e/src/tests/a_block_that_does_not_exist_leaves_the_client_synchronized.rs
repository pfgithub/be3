use super::*;

#[tokio::test]
async fn a_block_that_does_not_exist_leaves_the_client_synchronized() {
    let data_dir = std::env::temp_dir().join(format!("block-e2e-test-{}", Uuid::new_v4()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server_data_dir = data_dir.clone();
    let server = tokio::spawn(async move {
        block_server::serve(listener, server_data_dir)
            .await
            .unwrap();
    });
    let url = format!("http://{address}");
    let (account_id, token, workspace_id) = test_identity(&url).await;

    let client = BlockClient::new(account_id, workspace_id);
    client.connect(url, token);
    let missing = client.get_block::<Counter>(Uuid::new_v4());
    timeout(client.synchronized()).await;
    assert!(missing.read().is_none());

    let present = client.create_block(Counter { count: 0 });
    timeout(present.loaded()).await;
    present.operate(CounterOperation::Add(3));
    timeout(client.synchronized()).await;
    assert_eq!(present.read().unwrap().count, 3);

    drop(missing);
    drop(present);
    drop(client);
    server.abort();
    let _ = server.await;
    fs::remove_dir_all(data_dir).await.unwrap();
}
