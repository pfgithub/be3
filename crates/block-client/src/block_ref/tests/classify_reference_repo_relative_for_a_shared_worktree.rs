use super::*;

#[tokio::test]
async fn classify_reference_repo_relative_for_a_shared_worktree() {
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

    let first = client
        .classify_reference(referencing.id(), target.id(), &FakeWorktreeMembership)
        .await;
    let BlockRef::RepoRelative {
        repo: resolved_repo,
        eternal_id,
    } = first
    else {
        panic!("expected a repo-relative reference, got {first:?}");
    };
    assert_eq!(resolved_repo, repo);

    let second = client
        .classify_reference(referencing.id(), target.id(), &FakeWorktreeMembership)
        .await;
    assert_eq!(second, BlockRef::RepoRelative { repo, eternal_id });

    drop(worktree);
    drop(referencing);
    drop(target);
    drop(client);
    server.shutdown().await;
}
