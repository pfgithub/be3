use super::*;

#[tokio::test]
async fn classify_reference_direct_outside_a_worktree() {
    let server = TestServer::spawn().await;
    let (account_id, token, workspace_id) = identity(&server.url).await;
    let client = BlockClient::new(account_id, workspace_id);

    let referencing = client.create_block(Member);
    let target = client.create_block(Member);
    client.connect(server.url.clone(), token);
    referencing.loaded().await;
    target.loaded().await;
    referencing.set_parent(BlockParent::Root);
    target.set_parent(BlockParent::Root);
    client.synchronized().await;

    let reference = client
        .classify_reference(referencing.id(), target.id(), &FakeWorktreeMembership)
        .await;
    assert_eq!(reference, BlockRef::Direct(target.id()));

    drop(referencing);
    drop(target);
    drop(client);
    server.shutdown().await;
}
