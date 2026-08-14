use super::*;

#[tokio::test]
async fn version_control_worktree_membership_mints_and_resolves_eternal_ids() {
    let server = TestServer::spawn().await;
    let (account_id, token, workspace_id) = identity(&server.url).await;
    let client = BlockClient::new(account_id, workspace_id);

    // Blocks are created after `connect` (rather than the more common
    // create-then-connect order) because pending pre-connect creates flush
    // in an unordered batch; only a create issued while already connected is
    // guaranteed to reach the server before the next one, which matters here
    // since the worktree's own `references()` includes its `repo`.
    client.connect(server.url.clone(), token);
    let data_value = VersionControlData::new(account_id, 1_000);
    let data = client.create_block(data_value.clone());
    let worktree = client.create_block(VersionControlWorktree::new(data.id(), &data_value));
    let member = client.create_block(WorkspaceIndex::default());
    data.loaded().await;
    worktree.loaded().await;
    member.loaded().await;
    worktree.set_parent(BlockParent::Root);
    client.synchronized().await;

    let membership = VersionControlWorktreeMembership;
    assert_eq!(
        membership.worktree_type_id(),
        VersionControlWorktree::TYPE_ID
    );
    assert_eq!(membership.repo_id(&client, worktree.id()), Some(data.id()));
    assert_eq!(
        membership.eternal_id_for_member(&client, worktree.id(), member.id()),
        None
    );

    // Minting membership is what makes the worktree reference the member, so
    // it must happen before the member's own BlockParent can point back at
    // the worktree - the server rejects a parent link the parent doesn't
    // reference yet.
    let eternal_id = membership.mint_eternal_id(&client, worktree.id(), member.id());
    client.synchronized().await;
    member.set_parent(BlockParent::Uuid(worktree.id()));
    client.synchronized().await;

    assert_eq!(
        membership.eternal_id_for_member(&client, worktree.id(), member.id()),
        Some(eternal_id)
    );
    assert_eq!(
        membership.resolve_eternal_id(&client, worktree.id(), eternal_id),
        Some(member.id())
    );

    drop(data);
    drop(worktree);
    drop(member);
    drop(client);
    server.shutdown().await;
}
