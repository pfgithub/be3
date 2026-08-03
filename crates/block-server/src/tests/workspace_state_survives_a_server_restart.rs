use super::support::{create_workspace, management_request, register, TestServer};
use block::{ManagementClientMessage, ManagementServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn workspace_state_survives_a_server_restart() {
    let server = TestServer::start().await;
    let management = server.management();
    let owner = register(&management, "restart-owner@example.com").await;
    let workspace = create_workspace(&management, owner.id, "Persistent").await;
    let root = server.stop().await;

    let restarted = TestServer::start_at(root).await;
    let management = restarted.management();
    let response = management_request(
        &management,
        ManagementClientMessage::ListWorkspaces {
            request_id: Uuid::new_v4(),
            account_id: owner.id,
        },
    )
    .await;
    assert!(matches!(
        response,
        ManagementServerMessage::Workspaces { workspaces, .. } if workspaces == vec![workspace]
    ));
    restarted.cleanup().await;
}
