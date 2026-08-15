use super::*;

#[tokio::test]
async fn map_point_reference_resolves_to_none_when_unresolvable() {
    let server = TestServer::spawn().await;
    let (account_id, token, workspace_id) = identity(&server.url).await;
    let client = BlockClient::new(account_id, workspace_id);

    client.connect(server.url.clone(), token);
    let data_value = VersionControlData::new(account_id, 1_000);
    let data = client.create_block(data_value.clone());
    let worktree = client.create_block(VersionControlWorktree::new(data.id(), &data_value));
    let referencing = client.create_block(Map::new());
    data.loaded().await;
    worktree.loaded().await;
    referencing.loaded().await;
    worktree.set_parent(BlockParent::Root);
    client.synchronized().await;

    let membership = VersionControlWorktreeMembership;
    membership.mint_eternal_id(&client, worktree.id(), referencing.id());
    client.synchronized().await;
    referencing.set_parent(BlockParent::Uuid(worktree.id()));
    client.synchronized().await;

    let broken_reference = BlockRef::RepoRelative {
        repo: Uuid::new_v4(),
        eternal_id: Uuid::new_v4(),
    };
    let point = MapPoint::new(broken_reference, MapCoordinate::new(2.35, 48.85));
    referencing.operate(MapOperation::AddPoint { point });
    client.synchronized().await;
    assert!(referencing.read().unwrap().references().is_empty());

    let resolved = client
        .resolve_reference(referencing.id(), &broken_reference, &membership)
        .await;
    assert_eq!(resolved, None);

    drop(data);
    drop(worktree);
    drop(referencing);
    drop(client);
    server.shutdown().await;
}
