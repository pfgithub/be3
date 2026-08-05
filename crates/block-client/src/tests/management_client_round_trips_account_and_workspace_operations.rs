use crate::ManagementClient;
use block::WorkspaceRole;
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn management_client_round_trips_account_and_workspace_operations() {
    let root = std::env::temp_dir().join(format!("block-client-management-{}", Uuid::new_v4()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server_root = root.clone();
    let server = tokio::spawn(async move {
        block_server::serve(listener, server_root).await.unwrap();
    });
    let client = ManagementClient::new(format!(" {url}/ ")).unwrap();
    assert_eq!(client.url(), url);

    let owner = client
        .register("owner@example.com", "Owner", "owner-password")
        .await
        .unwrap();
    assert_eq!(
        client
            .login("OWNER@example.com", "owner-password")
            .await
            .unwrap()
            .account,
        owner.account
    );
    assert!(client
        .list_workspaces(&owner.token)
        .await
        .unwrap()
        .is_empty());
    let workspace = client
        .create_workspace(&owner.token, "Workspace")
        .await
        .unwrap();
    assert_eq!(
        client.list_workspaces(&owner.token).await.unwrap(),
        vec![workspace.clone()]
    );
    let invitation = client
        .invite(
            &owner.token,
            workspace.id,
            "recipient@example.com",
            WorkspaceRole::Administrator,
        )
        .await
        .unwrap();
    let recipient = client
        .register("recipient@example.com", "Recipient", "recipient-password")
        .await
        .unwrap();
    assert_eq!(
        client.list_invitations(&recipient.token).await.unwrap(),
        vec![invitation.clone()]
    );
    client
        .respond_invitation(&recipient.token, invitation.id, true)
        .await
        .unwrap();
    assert_eq!(
        client.list_workspaces(&recipient.token).await.unwrap()[0].id,
        workspace.id
    );

    server.abort();
    let _ = server.await;
    tokio::fs::remove_dir_all(root).await.unwrap();
}
