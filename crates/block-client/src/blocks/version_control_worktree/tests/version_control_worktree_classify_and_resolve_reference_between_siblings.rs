use super::*;

#[tokio::test]
async fn version_control_worktree_classify_and_resolve_reference_between_siblings() {
    let server = TestServer::spawn().await;
    let (account_id, token, workspace_id) = identity(&server.url).await;
    let client = BlockClient::new(account_id, workspace_id);

    client.connect(server.url.clone(), token);
    let data_value = VersionControlData::new(account_id, 1_000);
    let data = client.create_block(data_value.clone());
    let worktree = client.create_block(VersionControlWorktree::new(data.id(), &data_value));
    let referencing = client.create_block(WorkspaceIndex::default());
    let target = client.create_block(WorkspaceIndex::default());
    data.loaded().await;
    worktree.loaded().await;
    referencing.loaded().await;
    target.loaded().await;
    worktree.set_parent(BlockParent::Root);
    client.synchronized().await;

    let membership = VersionControlWorktreeMembership;
    let target_eternal_id = membership.mint_eternal_id(&client, worktree.id(), target.id());
    membership.mint_eternal_id(&client, worktree.id(), referencing.id());
    client.synchronized().await;
    referencing.set_parent(BlockParent::Uuid(worktree.id()));
    target.set_parent(BlockParent::Uuid(worktree.id()));
    client.synchronized().await;

    let reference = client
        .classify_reference(referencing.id(), target.id(), &membership)
        .await;
    let BlockRef::RepoRelative { repo, eternal_id } = reference else {
        panic!("expected a repo-relative reference, got {reference:?}");
    };
    assert_eq!(repo, data.id());
    assert_eq!(eternal_id, target_eternal_id);

    let resolved = client
        .resolve_reference(referencing.id(), &reference, &membership)
        .await;
    assert_eq!(resolved, Some(target.id()));

    drop(data);
    drop(worktree);
    drop(referencing);
    drop(target);
    drop(client);
    server.shutdown().await;
}
