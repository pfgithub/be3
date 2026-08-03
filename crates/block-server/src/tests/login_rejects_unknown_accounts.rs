use super::support::{management_request, TestServer};
use block::{ManagementClientMessage, ManagementErrorCode, ManagementServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn login_rejects_unknown_accounts() {
    let server = TestServer::start().await;
    let mut socket = server.connect_management().await;
    let response = management_request(
        &mut socket,
        ManagementClientMessage::Login {
            request_id: Uuid::new_v4(),
            email: "missing@example.com".into(),
        },
    )
    .await;
    assert!(matches!(
        response,
        ManagementServerMessage::Error {
            code: ManagementErrorCode::AccountNotFound,
            ..
        }
    ));
    server.cleanup().await;
}
