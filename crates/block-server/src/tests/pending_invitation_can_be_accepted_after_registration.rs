use super::support::{create_workspace, management_request, register, TestServer};
use block::{ManagementClientMessage, ManagementServerMessage, WorkspaceRole};
use uuid::Uuid;

#[tokio::test]
async fn pending_invitation_can_be_accepted_after_registration() {
    let server = TestServer::start().await;
    let management = server.management();
    let owner = register(&management, "owner@example.com").await;
    let workspace = create_workspace(&management, &owner.token, "Shared").await;
    let response = management_request(
        &management,
        ManagementClientMessage::Invite {
            request_id: Uuid::new_v4(),
            token: owner.token.clone(),
            workspace_id: workspace.id,
            email: " FUTURE@example.com ".into(),
            role: WorkspaceRole::Administrator,
        },
    )
    .await;
    let ManagementServerMessage::Invitation { invitation, .. } = response else {
        panic!("invite failed: {response:?}");
    };
    assert_eq!(invitation.email, "future@example.com");

    let recipient = register(&management, "future@example.com").await;
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
        ManagementServerMessage::Invitations { invitations, .. } if invitations == vec![invitation.clone()]
    ));
    assert!(matches!(
        management_request(
            &management,
            ManagementClientMessage::RespondInvitation {
                request_id: Uuid::new_v4(),
                token: recipient.token.clone(),
                invitation_id: invitation.id,
                accept: true,
            },
        )
        .await,
        ManagementServerMessage::Ok { .. }
    ));
    let response = management_request(
        &management,
        ManagementClientMessage::ListWorkspaces {
            request_id: Uuid::new_v4(),
            token: recipient.token.clone(),
        },
    )
    .await;
    assert!(matches!(
        response,
        ManagementServerMessage::Workspaces { workspaces, .. }
            if workspaces.len() == 1 && workspaces[0].id == workspace.id
    ));
    server.cleanup().await;
}
