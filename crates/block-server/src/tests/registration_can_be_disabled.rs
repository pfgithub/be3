use super::support::{management_request, Management};
use super::TEST_PASSWORD;
use block::{ManagementClientMessage, ManagementErrorCode, ManagementServerMessage};
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn registration_can_be_disabled() {
    let root = std::env::temp_dir().join(format!("block-server-test-{}", Uuid::new_v4()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server_root = root.clone();
    let config = crate::ServerConfig {
        allow_registration: false,
    };
    let task = tokio::spawn(async move {
        crate::serve_with_config(listener, server_root, config)
            .await
            .unwrap();
    });
    let management = Management::new(&url);

    let response = management_request(
        &management,
        ManagementClientMessage::Register {
            request_id: Uuid::new_v4(),
            email: "newcomer@example.com".into(),
            display_name: "Newcomer".into(),
            password: TEST_PASSWORD.into(),
        },
    )
    .await;
    assert!(matches!(
        response,
        ManagementServerMessage::Error {
            code: ManagementErrorCode::RegistrationDisabled,
            ..
        }
    ));

    // Accounts created some other way (the CLI's `add-account`, here stood in
    // for by registering directly against the store) can still log in.
    let store = crate::BlockStore::new(root.clone());
    store
        .register_account(
            "operator-added@example.com".into(),
            "Operator Added".into(),
            TEST_PASSWORD.into(),
        )
        .await
        .unwrap();
    let response = management_request(
        &management,
        ManagementClientMessage::Login {
            request_id: Uuid::new_v4(),
            email: "operator-added@example.com".into(),
            password: TEST_PASSWORD.into(),
        },
    )
    .await;
    assert!(matches!(response, ManagementServerMessage::Account { .. }));

    drop(store);
    task.abort();
    let _ = task.await;
    tokio::fs::remove_dir_all(root).await.unwrap();
}
