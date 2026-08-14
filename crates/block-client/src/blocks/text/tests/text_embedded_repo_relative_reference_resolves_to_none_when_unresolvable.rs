use block::Block;

use super::*;

#[tokio::test]
async fn text_embedded_repo_relative_reference_resolves_to_none_when_unresolvable() {
    let server = TestServer::spawn().await;
    let (account_id, token, workspace_id) = identity(&server.url).await;
    let client = BlockClient::new(account_id, workspace_id);

    client.connect(server.url.clone(), token);
    let data_value = VersionControlData::new(account_id, 1_000);
    let data = client.create_block(data_value.clone());
    let worktree = client.create_block(VersionControlWorktree::new(data.id(), &data_value));
    let referencing = client.create_block(TextDocument::new());
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

    let repo = Uuid::new_v4();
    let eternal_id = Uuid::new_v4();
    let broken_reference = BlockRef::RepoRelative { repo, eternal_id };
    let url = repo_relative_block_url(repo, eternal_id);
    let document = TextDocument::from_bytes(format!("see {url} for details"));
    assert!(document.references().is_empty());

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
