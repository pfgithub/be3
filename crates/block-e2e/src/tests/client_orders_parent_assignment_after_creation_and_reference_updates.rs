use super::*;

#[tokio::test]
async fn client_orders_parent_assignment_after_creation_and_reference_updates() {
    let root = std::env::temp_dir().join(format!("block-client-parent-test-{}", Uuid::new_v4()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let server_root = root.clone();
    let server = tokio::spawn(async move {
        block_server::serve(listener, server_root).await.unwrap();
    });

    let client = BlockClient::new(Uuid::new_v4());
    let parent = client.create_block(ReferencingBlock::default());
    let child = client.create_block(ReferencingBlock::default());
    client.connect(url);
    parent.loaded().await;
    child.loaded().await;
    parent.set_parent(BlockParent::Root);
    client.synchronized().await;

    assert_eq!(
        client
            .list_references(BlockReferenceList::Orphans)
            .await
            .into_iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![child.id()]
    );

    parent.operate(ReferenceOperation::Add(child.id()));
    child.set_parent(BlockParent::Uuid(parent.id()));
    client.synchronized().await;

    assert_eq!(
        client
            .list_references(BlockReferenceList::Roots)
            .await
            .into_iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![parent.id()]
    );

    drop(parent);
    drop(child);
    drop(client);
    server.abort();
    let _ = server.await;
    fs::remove_dir_all(root).await.unwrap();
}
