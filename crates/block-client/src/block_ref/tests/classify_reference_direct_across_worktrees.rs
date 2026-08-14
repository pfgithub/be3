use super::*;

#[tokio::test]
async fn classify_reference_direct_across_worktrees() {
    let server = TestServer::spawn().await;
    let (account_id, token, workspace_id) = identity(&server.url).await;
    let client = BlockClient::new(account_id, workspace_id);

    let first_worktree = client.create_block(FakeWorktree {
        repo: Uuid::new_v4(),
        children: Vec::new(),
        members: HashMap::new(),
    });
    let second_worktree = client.create_block(FakeWorktree {
        repo: Uuid::new_v4(),
        children: Vec::new(),
        members: HashMap::new(),
    });
    let referencing = client.create_block(Member);
    let target = client.create_block(Member);
    client.connect(server.url.clone(), token);
    first_worktree.loaded().await;
    second_worktree.loaded().await;
    referencing.loaded().await;
    target.loaded().await;
    first_worktree.set_parent(BlockParent::Root);
    second_worktree.set_parent(BlockParent::Root);
    first_worktree.operate(FakeWorktreeOperation::AddChild(referencing.id()));
    second_worktree.operate(FakeWorktreeOperation::AddChild(target.id()));
    referencing.set_parent(BlockParent::Uuid(first_worktree.id()));
    target.set_parent(BlockParent::Uuid(second_worktree.id()));
    client.synchronized().await;

    let reference = client
        .classify_reference(referencing.id(), target.id(), &FakeWorktreeMembership)
        .await;
    assert_eq!(reference, BlockRef::Direct(target.id()));

    drop(first_worktree);
    drop(second_worktree);
    drop(referencing);
    drop(target);
    drop(client);
    server.shutdown().await;
}
