use super::*;

#[tokio::test]
async fn crdt_text_clients_converge_after_concurrent_insertions() {
    let data_dir = std::env::temp_dir().join(format!("block-e2e-text-test-{}", Uuid::new_v4()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server_data_dir = data_dir.clone();
    let server = tokio::spawn(async move {
        block_server::serve(listener, server_data_dir)
            .await
            .unwrap();
    });
    let url = format!("ws://{address}");

    let client_a = BlockClient::new(Uuid::new_v4());
    client_a.connect(url.clone());
    let block_a = client_a.create_block(TextDocument::new());
    timeout(block_a.loaded()).await;

    let client_b = BlockClient::new(Uuid::new_v4());
    client_b.connect(url);
    let block_b = client_b.get_block::<TextDocument>(block_a.id());
    timeout(block_b.loaded()).await;

    let operation = {
        let document = block_a.read().unwrap();
        document.insert_operation(0, 'a').unwrap()
    };
    block_a.operate(operation);
    let operation = {
        let document = block_a.read().unwrap();
        document.insert_operation(1, 'b').unwrap()
    };
    block_a.operate(operation);
    let operation = {
        let document = block_b.read().unwrap();
        document.insert_operation(0, 'x').unwrap()
    };
    block_b.operate(operation);
    let operation = {
        let document = block_b.read().unwrap();
        document.insert_operation(1, 'y').unwrap()
    };
    block_b.operate(operation);
    timeout(client_a.synchronized()).await;
    timeout(client_b.synchronized()).await;
    timeout(block_a.wait_until(|document| document.len() == 4)).await;
    timeout(block_b.wait_until(|document| document.len() == 4)).await;

    let text_a = block_a.read().unwrap().text();
    let text_b = block_b.read().unwrap().text();
    assert_eq!(text_a, text_b);
    assert_eq!(text_a.chars().count(), 4);

    drop(block_a);
    drop(block_b);
    drop(client_a);
    drop(client_b);
    server.abort();
    let _ = server.await;
    fs::remove_dir_all(data_dir).await.unwrap();
}
