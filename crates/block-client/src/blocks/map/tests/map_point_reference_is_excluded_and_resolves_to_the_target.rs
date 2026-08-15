use super::*;

#[tokio::test]
async fn map_point_reference_is_excluded_and_resolves_to_the_target() {
    let server = TestServer::spawn().await;
    let (account_id, token, workspace_id) = identity(&server.url).await;
    let client = BlockClient::new(account_id, workspace_id);

    client.connect(server.url.clone(), token);
    let data_value = VersionControlData::new(account_id, 1_000);
    let data = client.create_block(data_value.clone());
    let worktree = client.create_block(VersionControlWorktree::new(data.id(), &data_value));
    let referencing = client.create_block(Map::new());
    let target = client.create_block(WorkspaceIndex::default());
    data.loaded().await;
    worktree.loaded().await;
    referencing.loaded().await;
    target.loaded().await;
    worktree.set_parent(BlockParent::Root);
    client.synchronized().await;

    let membership = VersionControlWorktreeMembership;
    membership.mint_eternal_id(&client, worktree.id(), referencing.id());
    membership.mint_eternal_id(&client, worktree.id(), target.id());
    client.synchronized().await;
    referencing.set_parent(BlockParent::Uuid(worktree.id()));
    target.set_parent(BlockParent::Uuid(worktree.id()));
    client.synchronized().await;

    let reference = client
        .classify_reference(referencing.id(), target.id(), &membership)
        .await;
    let BlockRef::RepoRelative { repo, .. } = reference else {
        panic!("expected a repo-relative reference, got {reference:?}");
    };
    assert_eq!(repo, data.id());

    let point = MapPoint::new(reference, MapCoordinate::new(2.35, 48.85));
    referencing.operate(MapOperation::AddPoint { point });
    client.synchronized().await;
    assert!(referencing.read().unwrap().references().is_empty());

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
