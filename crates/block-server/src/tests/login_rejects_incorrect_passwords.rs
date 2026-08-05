use super::support::{management_request, register_with_password, TestServer};
use block::{ManagementClientMessage, ManagementErrorCode, ManagementServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn login_rejects_incorrect_passwords() {
    let server = TestServer::start().await;
    let management = server.management();
    let account =
        register_with_password(&management, "wrongpass@example.com", "the-real-password").await;

    let response = management_request(
        &management,
        ManagementClientMessage::Login {
            request_id: Uuid::new_v4(),
            email: account.email.clone(),
            password: "not-the-real-password".into(),
        },
    )
    .await;
    assert!(matches!(
        response,
        ManagementServerMessage::Error {
            code: ManagementErrorCode::InvalidCredentials,
            ..
        }
    ));

    let response = management_request(
        &management,
        ManagementClientMessage::Login {
            request_id: Uuid::new_v4(),
            email: account.email.clone(),
            password: "the-real-password".into(),
        },
    )
    .await;
    assert!(matches!(response, ManagementServerMessage::Account { .. }));
    server.cleanup().await;
}
