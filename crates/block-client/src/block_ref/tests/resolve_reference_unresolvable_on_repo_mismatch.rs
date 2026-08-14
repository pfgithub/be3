use super::*;

#[tokio::test]
async fn resolve_reference_unresolvable_on_repo_mismatch() {
    let server = TestServer::spawn().await;
    let (account_id, token, workspace_id) = identity(&server.url).await;
    let client = BlockClient::new(account_id, workspace_id);

    let worktree = client.create_block(FakeWorktree {
        repo: Uuid::new_v4(),
        children: Vec::new(),
        members: HashMap::new(),
    });
    let referencing = client.create_block(Member);
    client.connect(server.url.clone(), token);
    worktree.loaded().await;
    referencing.loaded().await;
    worktree.set_parent(BlockParent::Root);
    worktree.operate(FakeWorktreeOperation::AddChild(referencing.id()));
    referencing.set_parent(BlockParent::Uuid(worktree.id()));
    client.synchronized().await;

    let reference = BlockRef::RepoRelative {
        repo: Uuid::new_v4(),
        eternal_id: Uuid::new_v4(),
    };
    let resolved = client
        .resolve_reference(referencing.id(), &reference, &FakeWorktreeMembership)
        .await;
    assert_eq!(resolved, None);

    drop(worktree);
    drop(referencing);
    drop(client);
    server.shutdown().await;
}
