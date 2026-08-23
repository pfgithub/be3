use super::support::{management_request, register, TestServer};
use block::{ManagementClientMessage, ManagementErrorCode, ManagementServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn logout_revokes_the_session_token() {
    let server = TestServer::start().await;
    let management = server.management();
    let account = register(&management, "logout@example.com").await;
    let workspace = super::support::create_workspace(&management, &account.token, "Logout").await;
    assert!(server
        .try_connect_to(&account.token, workspace.id)
        .await
        .is_ok());

    let response = management_request(
        &management,
        ManagementClientMessage::Logout {
            request_id: Uuid::new_v4(),
            token: account.token.clone(),
        },
    )
    .await;
    assert!(matches!(response, ManagementServerMessage::Ok { .. }));

    assert!(server
        .try_connect_to(&account.token, workspace.id)
        .await
        .is_err());
    let response = management_request(
        &management,
        ManagementClientMessage::ListWorkspaces {
            request_id: Uuid::new_v4(),
            token: account.token.clone(),
        },
    )
    .await;
    assert!(matches!(
        response,
        ManagementServerMessage::Error {
            code: ManagementErrorCode::InvalidToken,
            ..
        }
    ));

    let response = management_request(
        &management,
        ManagementClientMessage::Logout {
            request_id: Uuid::new_v4(),
            token: account.token.clone(),
        },
    )
    .await;
    assert!(matches!(response, ManagementServerMessage::Ok { .. }));
    server.cleanup().await;
}
