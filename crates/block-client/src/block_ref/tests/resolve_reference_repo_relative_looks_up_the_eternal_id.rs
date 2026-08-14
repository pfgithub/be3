use super::*;

#[tokio::test]
async fn resolve_reference_repo_relative_looks_up_the_eternal_id() {
    let server = TestServer::spawn().await;
    let (account_id, token, workspace_id) = identity(&server.url).await;
    let client = BlockClient::new(account_id, workspace_id);

    let repo = Uuid::new_v4();
    let worktree = client.create_block(FakeWorktree {
        repo,
        children: Vec::new(),
        members: HashMap::new(),
    });
    let referencing = client.create_block(Member);
    let target = client.create_block(Member);
    client.connect(server.url.clone(), token);
    worktree.loaded().await;
    referencing.loaded().await;
    target.loaded().await;
    worktree.set_parent(BlockParent::Root);
    worktree.operate(FakeWorktreeOperation::AddChild(referencing.id()));
    worktree.operate(FakeWorktreeOperation::AddChild(target.id()));
    referencing.set_parent(BlockParent::Uuid(worktree.id()));
    target.set_parent(BlockParent::Uuid(worktree.id()));
    client.synchronized().await;

    let reference = client
        .classify_reference(referencing.id(), target.id(), &FakeWorktreeMembership)
        .await;

    let resolved = client
        .resolve_reference(referencing.id(), &reference, &FakeWorktreeMembership)
        .await;
    assert_eq!(resolved, Some(target.id()));

    drop(worktree);
    drop(referencing);
    drop(target);
    drop(client);
    server.shutdown().await;
}
