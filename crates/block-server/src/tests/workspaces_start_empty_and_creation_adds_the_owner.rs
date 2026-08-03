use super::support::{create_workspace, management_request, register, TestServer};
use block::{ManagementClientMessage, ManagementServerMessage, WorkspaceRole};
use uuid::Uuid;

#[tokio::test]
async fn workspaces_start_empty_and_creation_adds_the_owner() {
    let server = TestServer::start().await;
    let mut socket = server.connect_management().await;
    let account = register(&mut socket, "owner@example.com").await;
    let response = management_request(
        &mut socket,
        ManagementClientMessage::ListWorkspaces {
            request_id: Uuid::new_v4(),
            account_id: account.id,
        },
    )
    .await;
    assert!(matches!(
        response,
        ManagementServerMessage::Workspaces { workspaces, .. } if workspaces.is_empty()
    ));

    let workspace = create_workspace(&mut socket, account.id, " Project ").await;
    assert_eq!(workspace.name, "Project");
    assert_eq!(workspace.owner_id, account.id);
    assert_eq!(workspace.role, WorkspaceRole::Administrator);
    let response = management_request(
        &mut socket,
        ManagementClientMessage::ListWorkspaces {
            request_id: Uuid::new_v4(),
            account_id: account.id,
        },
    )
    .await;
    assert!(matches!(
        response,
        ManagementServerMessage::Workspaces { workspaces, .. } if workspaces == vec![workspace]
    ));
    server.cleanup().await;
}
