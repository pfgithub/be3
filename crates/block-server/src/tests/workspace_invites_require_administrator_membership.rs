use super::support::{create_workspace, management_request, register, TestServer};
use block::{ManagementClientMessage, ManagementErrorCode, ManagementServerMessage, WorkspaceRole};
use uuid::Uuid;

#[tokio::test]
async fn workspace_invites_require_administrator_membership() {
    let server = TestServer::start().await;
    let management = server.management();
    let owner = register(&management, "owner@example.com").await;
    let stranger = register(&management, "stranger@example.com").await;
    let workspace = create_workspace(&management, &owner.token, "Private").await;
    let response = management_request(
        &management,
        ManagementClientMessage::Invite {
            request_id: Uuid::new_v4(),
            token: stranger.token.clone(),
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
