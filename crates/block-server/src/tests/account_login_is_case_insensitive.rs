use super::support::{management_request, TestServer};
use super::TEST_PASSWORD;
use block::{ManagementClientMessage, ManagementServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn account_login_is_case_insensitive() {
    let server = TestServer::start().await;
    let management = server.management();
    let registered = management_request(
        &management,
        ManagementClientMessage::Register {
            request_id: Uuid::new_v4(),
            email: "  Person@Example.COM ".into(),
            display_name: " Person ".into(),
            password: TEST_PASSWORD.into(),
        },
    )
    .await;
    let ManagementServerMessage::Account { account, .. } = registered else {
        panic!("registration failed: {registered:?}");
    };
    assert_eq!(account.email, "person@example.com");
    assert_eq!(account.display_name, "Person");

    let logged_in = management_request(
        &management,
        ManagementClientMessage::Login {
            request_id: Uuid::new_v4(),
            email: "PERSON@example.com".into(),
            password: TEST_PASSWORD.into(),
        },
    )
    .await;
    assert!(matches!(
        logged_in,
        ManagementServerMessage::Account { account: found, .. } if found == account
    ));
    server.cleanup().await;
}
