use super::support::{create_workspace, management_request, register, TestServer};
use block::{ManagementClientMessage, ManagementErrorCode, ManagementServerMessage, WorkspaceRole};
use uuid::Uuid;

#[tokio::test]
async fn workspace_invites_require_administrator_membership() {
    let server = TestServer::start().await;
    let mut socket = server.connect_management().await;
    let owner = register(&mut socket, "owner@example.com").await;
    let stranger = register(&mut socket, "stranger@example.com").await;
    let workspace = create_workspace(&mut socket, owner.id, "Private").await;
    let response = management_request(
        &mut socket,
        ManagementClientMessage::Invite {
            request_id: Uuid::new_v4(),
            account_id: stranger.id,
            workspace_id: workspace.id,
            email: "target@example.com".into(),
            role: WorkspaceRole::Administrator,
        },
    )
    .await;
    assert!(matches!(
        response,
        ManagementServerMessage::Error {
            code: ManagementErrorCode::PermissionDenied,
            ..
        }
    ));
    server.cleanup().await;
}
