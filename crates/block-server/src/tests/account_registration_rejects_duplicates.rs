use super::support::{management_request, TestServer};
use block::{ManagementClientMessage, ManagementErrorCode, ManagementServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn account_registration_rejects_duplicates() {
    let server = TestServer::start().await;
    let mut socket = server.connect_management().await;
    for email in ["duplicate@example.com", " DUPLICATE@example.com "] {
        let response = management_request(
            &mut socket,
            ManagementClientMessage::Register {
                request_id: Uuid::new_v4(),
                email: email.into(),
                display_name: "Duplicate".into(),
            },
        )
        .await;
        if email.starts_with(' ') {
            assert!(matches!(
                response,
                ManagementServerMessage::Error {
                    code: ManagementErrorCode::EmailAlreadyRegistered,
                    ..
                }
            ));
        } else {
            assert!(matches!(response, ManagementServerMessage::Account { .. }));
        }
    }
    server.cleanup().await;
}
