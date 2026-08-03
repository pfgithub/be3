use super::support::{create_workspace, management_request, register, TestServer};
use block::{ManagementClientMessage, ManagementServerMessage, WorkspaceRole};
use uuid::Uuid;

#[tokio::test]
async fn pending_invitation_can_be_declined() {
    let server = TestServer::start().await;
    let mut socket = server.connect_management().await;
    let owner = register(&mut socket, "owner@example.com").await;
    let recipient = register(&mut socket, "recipient@example.com").await;
    let workspace = create_workspace(&mut socket, owner.id, "Decline").await;
    let response = management_request(
        &mut socket,
        ManagementClientMessage::Invite {
            request_id: Uuid::new_v4(),
            account_id: owner.id,
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
        &mut socket,
        ManagementClientMessage::RespondInvitation {
            request_id: Uuid::new_v4(),
            account_id: recipient.id,
            invitation_id: invitation.id,
            accept: false,
        },
    )
    .await;
    let response = management_request(
        &mut socket,
        ManagementClientMessage::ListInvitations {
            request_id: Uuid::new_v4(),
            account_id: recipient.id,
        },
    )
    .await;
    assert!(matches!(
        response,
        ManagementServerMessage::Invitations { invitations, .. } if invitations.is_empty()
    ));
    server.cleanup().await;
}
