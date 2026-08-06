use crate::{
    crypto::{AccountKeypair, AccountPublicKey, WorkspaceKey},
    ManagementClient,
};
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn management_client_round_trips_workspace_key_grants() {
    let root = std::env::temp_dir().join(format!("block-client-key-grant-{}", Uuid::new_v4()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server_root = root.clone();
    let server = tokio::spawn(async move {
        block_server::serve(listener, server_root).await.unwrap();
    });
    let client = ManagementClient::new(url).unwrap();

    let owner_keypair = AccountKeypair::generate();
    let owner = client
        .register(
            "owner@example.com",
            "Owner",
            "owner-password",
            &owner_keypair,
        )
        .await
        .unwrap();
    let workspace = client
        .create_workspace(&owner.token, "Shared")
        .await
        .unwrap();

    // Nobody has been granted the key yet.
    assert_eq!(
        client
            .get_workspace_key(&owner.token, workspace.id)
            .await
            .unwrap(),
        None
    );

    // The creator bootstraps their own access.
    let workspace_key = WorkspaceKey::generate();
    let owner_wrapped = workspace_key.seal_for(&owner_keypair.public_key());
    client
        .submit_workspace_key_grant(&owner.token, workspace.id, owner.account.id, owner_wrapped)
        .await
        .unwrap();
    let fetched = client
        .get_workspace_key(&owner.token, workspace.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        WorkspaceKey::unseal(&owner_keypair, &fetched)
            .unwrap()
            .to_bytes(),
        workspace_key.to_bytes()
    );

    // Invite and accept a second member -- they have no key yet.
    let recipient_keypair = AccountKeypair::generate();
    let invitation = client
        .invite(
            &owner.token,
            workspace.id,
            "recipient@example.com",
            block::WorkspaceRole::Editor,
        )
        .await
        .unwrap();
    let recipient = client
        .register(
            "recipient@example.com",
            "Recipient",
            "recipient-password",
            &recipient_keypair,
        )
        .await
        .unwrap();
    client
        .respond_invitation(&recipient.token, invitation.id, true)
        .await
        .unwrap();
    assert_eq!(
        client
            .get_workspace_key(&recipient.token, workspace.id)
            .await
            .unwrap(),
        None
    );

    // The owner notices the pending grant and fulfils it.
    let grants = client.list_pending_key_grants(&owner.token).await.unwrap();
    assert_eq!(grants.len(), 1);
    let grant = &grants[0];
    assert_eq!(grant.workspace_id, workspace.id);
    assert_eq!(grant.account_id, recipient.account.id);
    let recipient_public_key = AccountPublicKey::from_slice(&grant.account_public_key).unwrap();
    let recipient_wrapped = workspace_key.seal_for(&recipient_public_key);
    client
        .submit_workspace_key_grant(
            &owner.token,
            workspace.id,
            recipient.account.id,
            recipient_wrapped,
        )
        .await
        .unwrap();
    assert!(client
        .list_pending_key_grants(&owner.token)
        .await
        .unwrap()
        .is_empty());

    // The recipient can now recover the exact same workspace key.
    let fetched = client
        .get_workspace_key(&recipient.token, workspace.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        WorkspaceKey::unseal(&recipient_keypair, &fetched)
            .unwrap()
            .to_bytes(),
        workspace_key.to_bytes()
    );

    server.abort();
    let _ = server.await;
    tokio::fs::remove_dir_all(root).await.unwrap();
}
