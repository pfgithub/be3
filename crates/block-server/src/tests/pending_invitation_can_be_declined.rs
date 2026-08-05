use super::support::{create_workspace, management_request, register, TestServer};
use block::{ManagementClientMessage, ManagementServerMessage, WorkspaceRole};
use uuid::Uuid;

#[tokio::test]
async fn pending_invitation_can_be_declined() {
    let server = TestServer::start().await;
    let management = server.management();
    let owner = register(&management, "owner@example.com").await;
    let recipient = register(&management, "recipient@example.com").await;
    let workspace = create_workspace(&management, &owner.token, "Decline").await;
    let response = management_request(
        &management,
        ManagementClientMessage::Invite {
            request_id: Uuid::new_v4(),
            token: owner.token.clone(),
            workspace_id: workspace.id,
            email: recipient.email.clone(),
            role: WorkspaceRole::Administrator,
        },
    )
    .await;
    let ManagementServerMessage::Invitation { invitation, .. } = response else {
        panic!("invite failed: {response:?}");
    };
    management_request(
        &management,
        ManagementClientMessage::RespondInvitation {
            request_id: Uuid::new_v4(),
            token: recipient.token.clone(),
            invitation_id: invitation.id,
            accept: false,
        },
    )
    .await;
    let response = management_request(
        &management,
        ManagementClientMessage::ListInvitations {
            request_id: Uuid::new_v4(),
            token: recipient.token.clone(),
        },
    )
    .await;
    assert!(matches!(
        response,
        ManagementServerMessage::Invitations { invitations, .. } if invitations.is_empty()
    ));
    server.cleanup().await;
}
